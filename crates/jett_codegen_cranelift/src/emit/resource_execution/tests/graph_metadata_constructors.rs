use super::*;
use jett_comptime::checked_types::CheckedExpressionTypes;
use jett_mir::{
    Function, ResourceCarrierOperationRole as CarrierRole, ResourceCompletion,
    ResourceConstructorNode as Node,
};

const SOURCE: &str = r#"namespace app
function create_callback() returns function() returns string:
    string captured = "baked"
    return function() returns string: return captured
actor Keeper:
    mutable function(resource_probe.TestHandle) returns nothing dispose = resource_probe.descriptor()
    receive alive responds bool:
        respond true
struct Bundle:
    keeper: Keeper
    dispose: function(resource_probe.TestHandle) returns nothing
    callback: function() returns string
    count: int64
export function make() returns Bundle:
    return comptime Bundle(keeper: spawn Keeper(), dispose: resource_probe.descriptor(), callback: create_callback(), count: run 7)
function relay(selected: Bundle) returns Bundle:
    Bundle alias = selected
    return clone alias
function main() returns nothing:
    Bundle selected = make()
    Bundle alias = clone selected
    Bundle forwarded = relay(alias)
    function() returns string callback = forwarded.callback
    string observed = callback()
    bool alive = ask forwarded.keeper.alive
    return nothing
"#;

fn baked_source(checked: &Arc<CheckedResourceProgram>) -> Program {
    let evaluated = jett_comptime::evaluate_explicit_comptime_expressions_capture(
        checked.module(),
        checked.checked().reflection_metadata.clone(),
        Arc::new(CheckedExpressionTypes {
            resource_program: Some(checked.clone()),
            expressions: checked
                .checked()
                .type_map
                .iter()
                .map(|(span, ty)| (*span, checked.checked().interner.type_name(*ty)))
                .collect(),
            ..Default::default()
        }),
        Arc::new(HashMap::new()),
    );
    assert!(
        evaluated.diagnostics.is_empty(),
        "{:?}",
        evaluated.diagnostics
    );
    assert!(evaluated.debug_events.is_empty());
    let mut hir = jett_hir::lower_checked_resource_program(checked).unwrap();
    jett_hir::bake_required_values(
        &mut hir,
        &evaluated.values,
        &checked.checked().interner,
        &checked.checked().reflection_metadata,
        &checked.checked().method_value_definitions,
    )
    .unwrap();
    jett_hir::complete_value_conversions(&mut hir, &checked.checked().interner).unwrap();
    let types = &checked.checked().interner;
    let mut program = jett_mir::lower(&hir, types).unwrap();
    jett_mir::prepare_native_sequences(&mut program, types);
    jett_mir::prepare_native_uninhabited_sums(&mut program, types);
    jett_mir::prepare_native_generated_functions(&mut program, types);
    program
}

fn factory(program: &Program) -> &Function {
    program
        .functions
        .iter()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == "make"
        })
        .unwrap()
}

fn emit_baked(
    program: &Program,
    checked: &Arc<CheckedResourceProgram>,
    release: bool,
) -> Result<ObjectArtifact, CodegenError> {
    super::super::super::emit_for_triple(
        program,
        &checked.checked().interner,
        HOST,
        Some(entry(program)),
        CodegenOptions { optimize: release },
    )
}

fn constructor_sites(function: &Function) -> Vec<(usize, usize)> {
    function
        .blocks
        .iter()
        .enumerate()
        .flat_map(|(block, body)| {
            body.statements
                .iter()
                .enumerate()
                .filter_map(move |(index, statement)| {
                    matches!(statement.kind, StatementKind::ResourceConstructor(_))
                        .then_some((block, index))
                })
        })
        .collect()
}

#[test]
fn graph_metadata_constructor_keeps_actor_hook_and_primitive_callback_in_both_profiles() {
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let types = &checked.checked().interner;
        let program = baked_source(&checked);
        let make = factory(&program);
        let sites = constructor_sites(make);
        assert_eq!(sites.len(), 7, "Begin, four fields, Commit and End survive");
        let mut fields = Vec::new();
        let mut output = None;
        for (block, index) in sites {
            let StatementKind::ResourceConstructor(node) =
                &make.blocks[block].statements[index].kind
            else {
                unreachable!()
            };
            let proof = jett_mir::native_graph_storage_for_constructor(make, types, node).unwrap();
            assert_eq!(proof.ty(), make.return_type);
            assert!(
                jett_mir::native_graph_storage_for_constructor(make, types, &node.clone())
                    .is_none(),
                "a copied node cannot select the protected physical occurrence"
            );
            match node {
                Node::Stage { field, .. } => fields.push(*field),
                Node::Commit { output: local, .. } => output = Some(*local),
                _ => {}
            }
        }
        assert_eq!(fields, [0, 1, 2, 3]);
        let header = make.local(output.unwrap()).unwrap();
        assert!(jett_mir::native_graph_storage_for_local(make, types, header).is_some());
        assert!(jett_mir::native_graph_storage_for_local(make, types, &header.clone()).is_none());
        let ownership =
            jett_mir::validate_resource_ownership_for_entry(&program, types, entry(&program))
                .unwrap();
        assert!(ownership.function(make.id).is_none_or(|family| {
            family.carriers().is_none_or(|carrier| {
                !carrier.operations().iter().any(|operation| {
                    matches!(
                        operation.role(),
                        CarrierRole::BeginConstructor { .. }
                            | CarrierRole::ConstructorChild { .. }
                            | CarrierRole::CommitConstructor { .. }
                    )
                })
            })
        }));
        let relay = program
            .functions
            .iter()
            .find(|function| {
                function.identity.declaration.namespace == "app"
                    && function.identity.declaration.name == "relay"
            })
            .unwrap();
        let formal = &relay.params[0];
        assert!(jett_mir::native_graph_storage_for_formal(relay, types, formal).is_some());
        assert!(
            jett_mir::native_graph_storage_for_formal(relay, types, &formal.clone()).is_none(),
            "an equal-type copied formal cannot qualify a wire shape"
        );
        let main = &program.functions[entry(&program).index() as usize];
        let call = main.blocks.iter().flat_map(|block| &block.statements).find_map(|statement| {
            let StatementKind::Let { value, .. } = &statement.kind else { return None; };
            matches!(&value.kind, jett_hir::ExpressionKind::Call { function, .. } if *function == relay.id)
                .then_some(value)
        }).unwrap();
        assert!(jett_mir::native_graph_source_edge(main, types, call, 0).is_some());
        assert!(
            jett_mir::native_graph_source_edge(main, types, &call.clone(), 0).is_none(),
            "an equal-type copied whole call cannot qualify an ordinary actual"
        );
        let artifact = emit_baked(&program, &checked, release).unwrap();
        let (definitions, imports) = symbols(&artifact);
        assert!(
            definitions
                .iter()
                .any(|name| name == JETT_AOT_ENTRY_SYMBOL_V1)
        );
        for required in [
            "jett_rt_v1_actor_graph_begin",
            "jett_rt_v1_actor_graph_commit",
            "jett_rt_v1_resource_graph_descriptor_box",
            "jett_rt_v1_struct_new",
            "jett_rt_v1_struct_init",
            "jett_rt_v1_struct_init_scalar_task",
        ] {
            assert!(
                imports.iter().any(|name| name == required),
                "{required}: {imports:?}"
            );
        }
        for forbidden in [
            "jett_rt_v1_resource_carrier_construct_begin",
            "jett_rt_v1_resource_carrier_child",
            "jett_rt_v1_resource_carrier_commit",
        ] {
            assert!(
                !imports.iter().any(|name| name == forbidden),
                "metadata-only constructors must not enter {forbidden}"
            );
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Mutation {
    SameTypeOutput,
    OutputHeader,
    FieldIndex,
    FieldEndpoint,
    CopiedStage,
    MovedStage,
    MissingEnd,
    ChangedEnd,
}

fn forge(function: &mut Function, mutation: Mutation) {
    let sites = constructor_sites(function);
    let (stage_block, stage_index) = sites
        .iter()
        .copied()
        .find(|(block, index)| {
            matches!(
                function.blocks[*block].statements[*index].kind,
                StatementKind::ResourceConstructor(Node::Stage { .. })
            )
        })
        .unwrap();
    let (commit_block, commit_index) = sites
        .iter()
        .copied()
        .find(|(block, index)| {
            matches!(
                function.blocks[*block].statements[*index].kind,
                StatementKind::ResourceConstructor(Node::Commit { .. })
            )
        })
        .unwrap();
    let (end_block, end_index) = sites
        .iter()
        .copied()
        .find(|(block, index)| {
            matches!(
                function.blocks[*block].statements[*index].kind,
                StatementKind::ResourceConstructor(Node::End { .. })
            )
        })
        .unwrap();
    let StatementKind::ResourceConstructor(Node::Commit { output, .. }) =
        function.blocks[commit_block].statements[commit_index].kind
    else {
        unreachable!()
    };
    match mutation {
        Mutation::SameTypeOutput => {
            let ty = function.local(output).unwrap().ty;
            let alternative = function
                .locals
                .iter()
                .find(|header| header.ty == ty && header.id != output)
                .unwrap()
                .id;
            let StatementKind::ResourceConstructor(Node::Commit { output, .. }) =
                &mut function.blocks[commit_block].statements[commit_index].kind
            else {
                unreachable!()
            };
            *output = alternative;
        }
        Mutation::OutputHeader => {
            function.locals[output.index() as usize]
                .name
                .push_str("_forged");
        }
        Mutation::FieldIndex | Mutation::FieldEndpoint => {
            let StatementKind::ResourceConstructor(Node::Stage { field, value, .. }) =
                &mut function.blocks[stage_block].statements[stage_index].kind
            else {
                unreachable!()
            };
            match mutation {
                Mutation::FieldIndex => *field = (*field + 1) % 4,
                Mutation::FieldEndpoint => value.span.end += 1,
                _ => unreachable!(),
            }
        }
        Mutation::CopiedStage => {
            let copy = function.blocks[stage_block].statements[stage_index].clone();
            function.blocks[stage_block]
                .statements
                .insert(stage_index + 1, copy);
        }
        Mutation::MovedStage => {
            assert_eq!(stage_block, commit_block);
            let statement = function.blocks[stage_block].statements.remove(stage_index);
            function.blocks[stage_block]
                .statements
                .insert(commit_index - 1, statement);
        }
        Mutation::MissingEnd => {
            function.blocks[end_block].statements.remove(end_index);
        }
        Mutation::ChangedEnd => {
            let StatementKind::ResourceConstructor(Node::End { outcome, .. }) =
                &mut function.blocks[end_block].statements[end_index].kind
            else {
                unreachable!()
            };
            *outcome = ResourceCompletion::Abort;
        }
    }
}

#[test]
fn graph_metadata_constructor_forged_output_header_field_or_lifecycle_refuses_emission() {
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let original = baked_source(&checked);
        for mutation in [
            Mutation::SameTypeOutput,
            Mutation::OutputHeader,
            Mutation::FieldIndex,
            Mutation::FieldEndpoint,
            Mutation::CopiedStage,
            Mutation::MovedStage,
            Mutation::MissingEnd,
            Mutation::ChangedEnd,
        ] {
            let mut program = original.clone();
            let function = program
                .functions
                .iter_mut()
                .find(|function| {
                    function.identity.declaration.namespace == "app"
                        && function.identity.declaration.name == "make"
                })
                .unwrap();
            forge(function, mutation);
            for (block, index) in constructor_sites(function) {
                let StatementKind::ResourceConstructor(node) =
                    &function.blocks[block].statements[index].kind
                else {
                    unreachable!()
                };
                assert!(
                    jett_mir::native_graph_storage_for_constructor(
                        function,
                        &checked.checked().interner,
                        node
                    )
                    .is_none(),
                    "{mutation:?} must not retain metadata constructor authority"
                );
            }
            assert!(
                matches!(
                    emit_baked(&program, &checked, release),
                    Err(CodegenError::InvalidMir(_))
                ),
                "{mutation:?} must fail before object emission"
            );
        }
    }
}

#[test]
fn graph_metadata_and_occupied_resource_constructors_keep_separate_routes_in_both_profiles() {
    let source = SOURCE
        .replace(
            "function main() returns nothing:",
            r#"struct OwnedPair:
    first: resource_probe.TestHandle
    second: resource_probe.TestHandle
    marker: int64
function retire_pair(first: resource_probe.TestHandle, second: resource_probe.TestHandle) returns nothing:
    OwnedPair held = OwnedPair(second: second, first: first, marker: 17)
    return nothing
export function main(net: Network) returns nothing:
    use resource_probe"#,
        )
        .replace(
            "    bool alive = ask forwarded.keeper.alive",
            r#"    resource_probe.TestHandle first = resource_probe.create(view net, 1002) handle error:
        return nothing
    resource_probe.TestHandle second = resource_probe.create(view net, 1003) handle error:
        return nothing
    retire_pair(first, second)"#,
        );
    for release in [false, true] {
        let checked = checked(&source, release);
        let types = &checked.checked().interner;
        let program = baked_source(&checked);
        let occupied = program
            .functions
            .iter()
            .find(|function| {
                function.identity.declaration.namespace == "app"
                    && function.identity.declaration.name == "retire_pair"
            })
            .unwrap();
        let sites = constructor_sites(occupied);
        assert!(!sites.is_empty());
        for (block, index) in sites {
            let StatementKind::ResourceConstructor(node) =
                &occupied.blocks[block].statements[index].kind
            else {
                unreachable!()
            };
            assert!(
                jett_mir::native_graph_storage_for_constructor(occupied, types, node).is_none()
            );
        }
        let ownership =
            jett_mir::validate_resource_ownership_for_entry(&program, types, entry(&program))
                .unwrap();
        let carrier = ownership.function(occupied.id).unwrap().carriers().unwrap();
        assert!(
            carrier
                .operations()
                .iter()
                .any(|operation| matches!(operation.role(), CarrierRole::CommitConstructor { .. }))
        );
        let artifact = emit_baked(&program, &checked, release).unwrap();
        let (_, imports) = symbols(&artifact);
        for required in [
            "jett_rt_v1_actor_graph_begin",
            "jett_rt_v1_resource_graph_descriptor_box",
            "jett_rt_v1_resource_carrier_construct_begin",
            "jett_rt_v1_resource_carrier_child",
            "jett_rt_v1_resource_carrier_commit",
            "jett_rt_v1_resource_carrier_transfer",
        ] {
            assert!(
                imports.iter().any(|name| name == required),
                "{required}: {imports:?}"
            );
        }
    }
}

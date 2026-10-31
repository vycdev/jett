//! These controls are authored only. The graph worker runs the frozen packet.
use super::*;
use crate::resource_ownership::tests::{SUPPORT, checked_support};
use jett_comptime::checked_types::CheckedExpressionTypes;
use std::{collections::HashMap, sync::Arc};

const SOURCE: &str = r#"namespace app
actor Keeper:
    mutable function(resource_probe.TestHandle) returns nothing dispose = resource_probe.descriptor()
    receive alive responds bool:
        respond true
struct Bundle:
    keeper: Keeper
    dispose: function(resource_probe.TestHandle) returns nothing
export function make() returns Bundle:
    return comptime Bundle(keeper: spawn Keeper(), dispose: resource_probe.descriptor())
function relay(selected: Bundle) returns Bundle:
    Bundle alias = selected
    return clone alias
function main() returns nothing:
    Bundle selected = make()
    Bundle alias = clone selected
    Bundle forwarded = relay(alias)
    function(resource_probe.TestHandle) returns nothing dispose = forwarded.dispose
    bool alive = ask forwarded.keeper.alive
    return nothing
"#;
fn baked(
    source: &str,
    release: bool,
) -> (hir::Program, Arc<jett_typecheck::CheckedResourceProgram>) {
    let checked = checked_support(source, SUPPORT, release);
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
    let mut program = hir::lower_checked_resource_program(&checked).unwrap();
    hir::bake_required_values(
        &mut program,
        &evaluated.values,
        &checked.checked().interner,
        &checked.checked().reflection_metadata,
        &checked.checked().method_value_definitions,
    )
    .unwrap();
    hir::complete_value_conversions(&mut program, &checked.checked().interner).unwrap();
    (program, checked)
}
fn named<'a>(program: &'a hir::Program, name: &str) -> &'a hir::Function {
    program
        .functions
        .iter()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == name
        })
        .unwrap()
}
fn original(
    program: &hir::Program,
    checked: &jett_typecheck::CheckedResourceProgram,
    name: &str,
) -> Witness {
    capture(
        named(program, name),
        &program.resource_source,
        &checked.checked().interner,
    )
    .unwrap()
}
#[test]
fn rooted_export_alias_formal_return_and_hook_projection_keep_original_producers() {
    for release in [false, true] {
        let (program, checked) = baked(SOURCE, release);
        let make = original(&program, &checked, "make");
        assert!(make.original_storage_for_return().is_some());
        let relay = original(&program, &checked, "relay");
        assert!(
            relay
                .original_storage_for_formal(&named(&program, "relay").params[0])
                .is_some()
        );
        assert!(relay.original_storage_for_return().is_some());
        let main = original(&program, &checked, "main");
        let header = named(&program, "main")
            .locals
            .iter()
            .find(|local| local.name == "dispose")
            .unwrap();
        let proof = main.original_storage_for_local(header).unwrap();
        let projection = hook_projection(proof).unwrap();
        assert_eq!(projection.ty(), header.ty);
        assert_eq!(
            projection.hook().recipe(),
            jett_types::ResourceKernelRecipe::Finalize
        );
        assert!(!projection.producers().is_empty());
        for producer in projection.producers() {
            assert!(producer.projected_hook().unwrap().belongs_to(&checked));
            assert!(
                producer
                    .projected_hook()
                    .unwrap()
                    .required()
                    .belongs_to(&checked)
            );
        }
        assert!(main.edges.iter().any(|edge| edge.parameter_index() == 0
            && edge.original_formal() == &named(&program, "relay").params[0]));
        let handler = program
            .functions
            .iter()
            .find(|function| {
                function.identity.declaration.kind == hir::DeclarationKind::ActorHandler
            })
            .unwrap();
        let captures = capture(
            handler,
            &program.resource_source,
            &checked.checked().interner,
        )
        .unwrap();
        assert!(
            captures
                .original_storage_for_formal(&handler.params[0])
                .is_some()
        );
    }
}
#[test]
fn equal_signature_unknown_incoming_edge_cannot_inherit_a_graph_formal_or_return() {
    let source=SOURCE.replace("    Bundle forwarded = relay(alias)",
        "    Bundle raw = Bundle(keeper: spawn Keeper(), dispose: resource_probe.descriptor())\n    Bundle unproved = relay(raw)\n    Bundle forwarded = relay(alias)");
    let (program, checked) = baked(&source, false);
    let relay = original(&program, &checked, "relay");
    assert!(
        relay
            .original_storage_for_formal(&named(&program, "relay").params[0])
            .is_none()
    );
    assert!(relay.original_storage_for_return().is_none());
    // The independent authentic exported required root remains available.
    assert!(
        original(&program, &checked, "make")
            .original_storage_for_return()
            .is_some()
    );
}
#[test]
fn changed_original_owner_cache_and_mutable_capture_refuse_before_storage_selection() {
    let (program, checked) = baked(SOURCE, false);
    let owner = named(&program, "make");
    let mut changed = owner.clone();
    changed.body.statements.clear();
    assert!(
        capture(
            &changed,
            &program.resource_source,
            &checked.checked().interner
        )
        .is_err()
    );
    let (foreign, _) = baked(SOURCE, false);
    assert!(
        capture(
            named(&foreign, "make"),
            &program.resource_source,
            &checked.checked().interner
        )
        .is_err()
    );
    let source = SOURCE.replace(
        "        respond true",
        "        dispose = resource_probe.descriptor()\n        respond true",
    );
    let (changed, checked) = baked(&source, false);
    let handler = changed
        .functions
        .iter()
        .find(|function| function.identity.declaration.kind == hir::DeclarationKind::ActorHandler)
        .unwrap();
    let proof = capture(
        handler,
        &changed.resource_source,
        &checked.checked().interner,
    )
    .unwrap();
    assert!(
        proof
            .original_storage_for_formal(&handler.params[0])
            .is_none()
    );
}
#[test]
fn writer_in_another_handler_cannot_lend_a_frozen_hook_to_a_reader() {
    let source = SOURCE.replace("struct Bundle:",
        "    receive replace responds bool:\n        dispose = resource_probe.descriptor()\n        respond true\nstruct Bundle:");
    let (program, checked) = baked(&source, false);
    let reader = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.kind == hir::DeclarationKind::ActorHandler)
        .unwrap();
    let proof = capture(
        reader,
        &program.resource_source,
        &checked.checked().interner,
    )
    .unwrap();
    assert!(
        proof
            .original_storage_for_formal(&reader.params[0])
            .is_none()
    );
}

#[test]
fn mutable_capture_requires_authenticated_transitive_helper_preservation() {
    for (body, stable) in [
        ("    return true", true),
        (
            "    mutable bool selected = false\n    selected = true\n    return selected",
            false,
        ),
    ] {
        let source = SOURCE
            .replacen(
                "actor Keeper:",
                &format!("function observed() returns bool:\n{body}\nactor Keeper:"),
                1,
            )
            .replacen("        respond true", "        respond observed()", 1);
        let (program, checked) = baked(&source, false);
        let reader = program
            .functions
            .iter()
            .find(|function| {
                function.identity.declaration.kind == hir::DeclarationKind::ActorHandler
            })
            .unwrap();
        let witness = capture(
            reader,
            &program.resource_source,
            &checked.checked().interner,
        )
        .unwrap();
        assert_eq!(
            witness
                .original_storage_for_formal(&reader.params[0])
                .is_some(),
            stable
        );
    }
}

#[test]
fn required_actor_constructor_retires_only_without_a_retained_runtime_spawn() {
    let (program, _) = baked(SOURCE, false);
    let constructor = program
        .functions
        .iter()
        .find(|function| {
            function.identity.declaration.kind == hir::DeclarationKind::ActorConstructor
        })
        .unwrap();
    assert!(
        program
            .resource_source
            .required_only_function_ids()
            .contains(&constructor.id)
    );
    for handler in program
        .functions
        .iter()
        .filter(|function| function.identity.declaration.kind == hir::DeclarationKind::ActorHandler)
    {
        assert!(
            !program
                .resource_source
                .required_only_function_ids()
                .contains(&handler.id)
        );
    }
    let runtime = SOURCE.replacen(
        "    Bundle alias = clone selected",
        "    Keeper dynamic = spawn Keeper()\n    Bundle alias = clone selected",
        1,
    );
    let (program, _) = baked(&runtime, false);
    let constructor = program
        .functions
        .iter()
        .find(|function| {
            function.identity.declaration.kind == hir::DeclarationKind::ActorConstructor
        })
        .unwrap();
    assert!(
        !program
            .resource_source
            .required_only_function_ids()
            .contains(&constructor.id)
    );
}

#[test]
fn hidden_root_token_has_only_exact_scalar_storage_and_operand_rows() {
    let (program, checked) = baked(SOURCE, false);
    let owner = named(&program, "make");
    let witness = original(&program, &checked, "make");
    let token = witness
        .values
        .iter()
        .find(|row| matches!(row.proof.shape, Shape::HiddenRootToken { .. }))
        .unwrap();
    assert_eq!(token.proof.ty(), TypeInterner::UINT64);
    assert!(!token.proof.transferable());
    assert!(hook_projection(&token.proof).is_none());
    let fill_header = witness
        .headers
        .iter()
        .find(|row| row.proof == token.proof)
        .unwrap();
    let plan = match &token.proof.shape {
        Shape::HiddenRootToken { fill, .. } => fill,
        _ => unreachable!(),
    };
    assert!(matches!(
        plan.operation(),
        hir::ActorGraphOperation::RootFill { .. }
    ));
    let mut found = false;
    walk::hir_block(&owner.body, &mut |value| {
        if let E::ActorGraphImport(import) = &value.kind {
            for statement in &import.body().statements {
                let value = match &statement.kind {
                    hir::StatementKind::Let { value, .. }
                    | hir::StatementKind::Expression(value) => value,
                    _ => continue,
                };
                if let E::ActorGraphStep { instruction, args } = &value.kind
                    && matches!(
                        instruction.operation(),
                        hir::ActorGraphOperation::RootPublish { .. }
                    )
                {
                    assert!(plan.same_import(instruction));
                    assert!(
                        matches!(args[1].kind, E::Local(local) if local == fill_header.original.id)
                    );
                    let proof = witness.original_storage_for_expression(&args[1]).unwrap();
                    assert_eq!(proof, &token.proof);
                    assert!(
                        witness
                            .original_storage_for_expression(value)
                            .unwrap()
                            .transferable()
                    );
                    found = true;
                }
            }
        }
    });
    assert!(found);
    assert!(witness.returned.as_ref().unwrap().transferable());
}

#[test]
fn retained_float_bits_are_reflexive_and_keep_signed_zero_distinct() {
    let (program, checked) = baked(SOURCE, false);
    let witness = original(&program, &checked, "make");
    let plan = witness
        .values
        .iter()
        .find_map(|row| match &row.proof.shape {
            Shape::Actor { plan, .. } | Shape::Plain { plan, .. } => Some(plan.clone()),
            _ => None,
        })
        .unwrap();
    let value = Expression {
        kind: E::Float(f64::from_bits(0x7ff8_0000_0000_0011)),
        ty: TypeInterner::FLOAT64,
        span: named(&program, "make").span,
    };
    let proof = GraphStorageProof {
        ty: value.ty,
        shape: Shape::Plain {
            plan,
            value: value.clone(),
        },
    };
    assert_eq!(proof, proof.clone());
    let row = ValueRow {
        original: value.clone(),
        current: value.clone(),
        proof: proof.clone(),
    };
    assert_eq!(row, row.clone());
    let mut rows = Witness::default();
    rows.calls.push(value.clone());
    rows.values.push(row);
    assert_eq!(rows, rows.clone());
    let mut different = proof.clone();
    if let Shape::Plain { value, .. } = &mut different.shape {
        value.kind = E::Float(f64::from_bits(0x7ff8_0000_0000_0012));
    }
    assert_ne!(proof, different);
    let mut positive = proof.clone();
    let mut negative = proof;
    if let Shape::Plain { value, .. } = &mut positive.shape {
        value.kind = E::Float(0.0);
    }
    if let Shape::Plain { value, .. } = &mut negative.shape {
        value.kind = E::Float(-0.0);
    }
    assert_ne!(positive, negative);
}
#[test]
fn raw_function_and_occupied_resource_types_do_not_mint_storage_rows() {
    let (program, checked) = baked(SOURCE, false);
    let types = &checked.checked().interner;
    let proof = original(&program, &checked, "main");
    let header = named(&program, "main")
        .locals
        .iter()
        .find(|local| local.name == "dispose")
        .unwrap();
    let synthetic = Expression {
        kind: E::FunctionRef(named(&program, "main").id),
        ty: header.ty,
        span: header.span,
    };
    assert!(proof.original_storage_for_expression(&synthetic).is_none());
    assert!(storage_safe(types, header.ty)); // Query alone conveys no proof.
    let resource = types.resolve(header.ty);
    let Type::Function { params, .. } = resource else {
        panic!("stored hook signature")
    };
    assert!(!storage_safe(types, params[0]));
    let network = types
        .type_ids()
        .find(|id| matches!(types.resolve(*id), Type::Capability(_)));
    if let Some(network) = network {
        assert!(!storage_safe(types, network));
    }
}
#[test]
fn current_body_header_pointer_and_resealed_parent_forgeries_refuse() {
    for release in [false, true] {
        let (hir, checked) = baked(SOURCE, release);
        let types = &checked.checked().interner;
        let program = crate::lower(&hir, types).unwrap();
        let function = program
            .functions
            .iter()
            .find(|function| {
                function.identity.declaration.namespace == "app"
                    && function.identity.declaration.name == "main"
            })
            .unwrap();
        let parent = function.resource_lowering.as_ref().unwrap();
        validate(parent, function, types).unwrap();
        let expression = function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .find_map(|statement| match &statement.kind {
                StatementKind::Let { value, .. } if matches!(value.kind, E::Field { .. }) => {
                    Some(value)
                }
                _ => None,
            })
            .unwrap();
        assert!(storage_for_expression(parent, function, types, expression).is_some());
        assert!(storage_for_expression(parent, function, types, &expression.clone()).is_none());
        let mut changed = function.clone();
        changed.blocks[0].statements.clear();
        changed.resource_lowering.as_mut().unwrap().blocks = changed.blocks.clone();
        assert!(validate(changed.resource_lowering.as_ref().unwrap(), &changed, types).is_err());
        let mut changed = function.clone();
        changed.locals[0].name.push_str("foreign");
        changed.resource_lowering.as_mut().unwrap().locals = changed.locals.clone();
        assert!(validate(changed.resource_lowering.as_ref().unwrap(), &changed, types).is_err());
        let cloned_header = function
            .locals
            .iter()
            .find(|local| {
                parent
                    .graph_metadata
                    .original_storage_for_local(local)
                    .is_some()
            })
            .unwrap()
            .clone();
        assert!(storage_for_local(parent, function, types, &cloned_header).is_none());

        let factory = program
            .functions
            .iter()
            .find(|function| {
                function.identity.declaration.namespace == "app"
                    && function.identity.declaration.name == "make"
            })
            .unwrap();
        let parent = factory.resource_lowering.as_ref().unwrap();
        let ownership = crate::validate_resource_ownership(&program, types).unwrap();
        let plan = ownership.function(factory.id).unwrap();
        assert!(
            plan.carriers().is_none(),
            "protected constructor retains ordinary storage"
        );
        assert_eq!(
            plan.frames().len(),
            1,
            "no constructor or return custody frames"
        );
        assert!(plan.owner_slots().is_empty());
        assert!(plan.loans().is_empty());
        assert!(
            plan.graph_descriptor_operations().next().is_some(),
            "hook Descriptor rows survive staging"
        );
        let output = parent.graph_metadata.constructor_outputs.first().unwrap();
        let header = factory.local(output.current.id).unwrap();
        let companion = crate::ResourceCompanionPlan::analyze(&ownership, factory.id).unwrap();
        assert!(
            companion
                .storage()
                .owned_locals
                .contains(&(header.id.index() as usize))
        );
        assert!(
            companion.storage().temporary_slots > 0,
            "boxed Graph producer owns a temporary"
        );
        for current in program.functions.iter().filter(|current| {
            current.identity.declaration.namespace == "app"
                && matches!(current.identity.declaration.name.as_str(), "relay" | "main")
        }) {
            crate::ResourceCompanionPlan::analyze(&ownership, current.id).unwrap();
        }
        assert!(
            crate::copy_values::CopyValuePlan::analyze(factory, types)
                .unwrap_err()
                .contains("pending ResourceOwnershipPlan")
        );
        assert!(storage_for_local(parent, factory, types, header).is_some());
        assert!(storage_for_local(parent, factory, types, &header.clone()).is_none());
        let nodes = factory
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .filter_map(|statement| match &statement.kind {
                StatementKind::ResourceConstructor(node) => Some(node),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(nodes.len(), 5, "Begin, two fields, Commit and End survive");
        for node in &nodes {
            assert!(crate::native_graph_storage_for_constructor(factory, types, node).is_some());
            assert!(
                crate::native_graph_storage_for_constructor(factory, types, &(*node).clone())
                    .is_none()
            );
        }
        let mut changed = factory.clone();
        changed
            .resource_lowering
            .as_mut()
            .unwrap()
            .graph_metadata
            .constructor_outputs
            .clear();
        assert!(crate::native_graph_storage_for_return(&changed, types).is_none());
        assert!(
            crate::native_graph_storage_for_local(
                &changed,
                types,
                changed.local(header.id).unwrap()
            )
            .is_none()
        );
        let mut missing = program.clone();
        missing.functions[factory.id.index() as usize] = changed;
        assert!(crate::validate_resource_ownership(&missing, types).is_err());
        let mut changed = factory.clone();
        let alternative = factory
            .locals
            .iter()
            .find(|local| local.ty == header.ty && local.id != header.id)
            .unwrap();
        changed
            .resource_lowering
            .as_mut()
            .unwrap()
            .graph_metadata
            .constructor_outputs[0]
            .current = alternative.clone();
        assert!(validate(changed.resource_lowering.as_ref().unwrap(), &changed, types).is_err());
        let mut changed = factory.clone();
        changed.resource_lowering.as_mut().unwrap().constructor_seal = Arc::from([]);
        assert!(validate(changed.resource_lowering.as_ref().unwrap(), &changed, types).is_err());
        let mut changed = factory.clone();
        let foreign = parent
            .graph_metadata
            .values
            .iter()
            .find(|row| row.proof != output.proof)
            .unwrap();
        changed
            .resource_lowering
            .as_mut()
            .unwrap()
            .graph_metadata
            .constructor_outputs[0]
            .proof = foreign.proof.clone();
        assert!(validate(changed.resource_lowering.as_ref().unwrap(), &changed, types).is_err());
        let mut changed = factory.clone();
        let stage = changed
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.statements)
            .find_map(|statement| match &mut statement.kind {
                StatementKind::ResourceConstructor(ResourceConstructorNode::Stage {
                    value,
                    ..
                }) => Some(value),
                _ => None,
            })
            .unwrap();
        stage.span.end += 1;
        changed.resource_lowering.as_mut().unwrap().blocks = changed.blocks.clone();
        assert!(validate(changed.resource_lowering.as_ref().unwrap(), &changed, types).is_err());
    }
}

#[test]
fn batched_original_graph_analysis_keeps_exact_archive_owner_and_type_meanings() {
    for release in [false, true] {
        let (hir, checked) = baked(SOURCE, release);
        let types = &checked.checked().interner;
        let analysis = OriginalGraphAnalysis::prepare(&hir.resource_source, types).unwrap();
        let make = named(&hir, "make");
        let first = analysis.capture(make, &hir.resource_source, types).unwrap();
        let second = analysis.capture(make, &hir.resource_source, types).unwrap();
        assert!(
            std::ptr::eq(first, second),
            "one immutable row serves repeated selection"
        );
        assert!(first.original_storage_for_return().is_some());
        assert!(
            analysis
                .capture(named(&hir, "relay"), &hir.resource_source, types)
                .unwrap()
                .original_storage_for_return()
                .is_some()
        );

        let mut edited = make.clone();
        edited.body.statements.clear();
        assert!(
            analysis
                .capture(&edited, &hir.resource_source, types)
                .is_err()
        );
        let mut edited = make.clone();
        edited.locals[0].name.push_str("foreign");
        assert!(
            analysis
                .capture(&edited, &hir.resource_source, types)
                .is_err()
        );
        let (foreign, foreign_checked) = baked(SOURCE, release);
        assert!(
            analysis
                .capture(
                    named(&foreign, "make"),
                    &foreign.resource_source,
                    &foreign_checked.checked().interner
                )
                .is_err()
        );

        // The original TypeIds alone cannot replace their nominal definitions.
        let mut missing = TypeInterner::new();
        for ty in types.type_ids() {
            assert_eq!(missing.intern(types.resolve(ty).clone()), ty);
        }
        assert!(
            analysis
                .capture(make, &hir.resource_source, &missing)
                .is_err()
        );

        let program = crate::lower(&hir, types).unwrap();
        let mut current = program
            .functions
            .iter()
            .find(|function| {
                function.identity.declaration.namespace == "app"
                    && function.identity.declaration.name == "make"
            })
            .unwrap()
            .clone();
        current.resource_lowering.as_mut().unwrap().execution =
            authenticate_original(&foreign, &foreign_checked.checked().interner).unwrap();
        assert!(
            validate(current.resource_lowering.as_ref().unwrap(), &current, types).is_err(),
            "equal-text foreign analysis cannot authenticate current rows"
        );
    }
}

#[test]
fn completed_empty_graph_analysis_keeps_resource_custody_and_general_current_validation() {
    const ORDINARY: &str = r#"namespace app
function lifecycle(view net: Network) returns nothing:
    use resource_probe
    resource_probe.TestHandle token = resource_probe.create(view net, 7) handle error:
        return nothing
    resource_probe.close(token)
    return nothing
function main(net: Network) returns nothing:
    lifecycle(view net)
    return nothing
"#;
    for release in [false, true] {
        let checked = checked_support(ORDINARY, SUPPORT, release);
        let types = &checked.checked().interner;
        let hir = hir::lower_checked_resource_program(&checked).unwrap();
        let execution = authenticate_original(&hir, types).unwrap();
        assert!(execution.has_no_original_graph_rows(&hir.resource_source));
        let (foreign, _) = baked(SOURCE, release);
        assert!(!execution.has_no_original_graph_rows(&foreign.resource_source));
        assert!(
            !ResourceExecutionClosure::default().has_no_original_graph_rows(&hir.resource_source)
        );

        let program = crate::lower(&hir, types).unwrap();
        let plan = crate::validate_resource_ownership(&program, types).unwrap();
        let entry = program
            .functions
            .iter()
            .find(|function| {
                function.identity.declaration.namespace == "app"
                    && function.identity.declaration.name == "main"
            })
            .unwrap();
        assert!(
            plan.function(entry.id).is_some(),
            "ordinary Resource Scope remains mandatory"
        );
        for function in &program.functions {
            if let Some(parent) = &function.resource_lowering {
                assert!(!has_records(parent, function, types));
                assert!(storage_for_return(parent, function, types).is_none());
                for local in &function.locals {
                    assert!(storage_for_local(parent, function, types, local).is_none());
                }
                for formal in &function.params {
                    assert!(storage_for_formal(parent, function, types, formal).is_none());
                }
                for block in &function.blocks {
                    walk::mir_block(block, &mut |expression| {
                        assert!(
                            storage_for_expression(parent, function, types, expression).is_none()
                        );
                        assert!(source_edge(parent, function, types, expression, 0).is_none());
                    });
                }
            }
        }
        let mut edited = program.clone();
        edited.functions[entry.id.index() as usize].locals[0]
            .name
            .push_str("foreign");
        assert!(
            crate::validate_resource_ownership(&edited, types).is_err(),
            "negative metadata answers cannot authenticate an edited Resource header"
        );
        let mut edited = program.clone();
        edited.functions[entry.id.index() as usize].blocks.clear();
        assert!(
            crate::validate_resource_ownership(&edited, types).is_err(),
            "general current Resource/CFG validation still rejects edited bodies"
        );
    }
}

#[test]
fn positive_graph_analysis_never_enters_completed_empty_fastpath() {
    for release in [false, true] {
        let (hir, checked) = baked(SOURCE, release);
        let types = &checked.checked().interner;
        let execution = authenticate_original(&hir, types).unwrap();
        assert!(!execution.has_no_original_graph_rows(&hir.resource_source));
        let program = crate::lower(&hir, types).unwrap();
        let make = program
            .functions
            .iter()
            .find(|function| {
                function.identity.declaration.namespace == "app"
                    && function.identity.declaration.name == "make"
            })
            .unwrap();
        let parent = make.resource_lowering.as_ref().unwrap();
        assert!(!no_original_or_current_records(parent));
        assert!(storage_for_return(parent, make, types).is_some());
        let output = &parent.graph_metadata.constructor_outputs[0];
        let header = make.local(output.current.id).unwrap();
        assert!(storage_for_local(parent, make, types, header).is_some());
        assert!(storage_for_local(parent, make, types, &header.clone()).is_none());
        let mut edited = make.clone();
        edited.locals[header.id.index() as usize]
            .name
            .push_str("foreign");
        assert!(
            storage_for_return(edited.resource_lowering.as_ref().unwrap(), &edited, types)
                .is_none()
        );
    }
}

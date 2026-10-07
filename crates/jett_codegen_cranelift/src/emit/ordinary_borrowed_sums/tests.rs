use super::*;
use jett_common::{FileId, SourceOrigin};
use std::collections::HashMap;

const SOURCE: &str = r#"namespace app
function project_optional(view source: optional[list[int64]]) returns int64:
    list[int64] alias = view((view source) handle:
        return 0
    )
    list[int64] forwarded = alias
    return 1
function project_result(view source: result[list[int64], string]) returns int64:
    list[int64] alias = view((view source) handle error:
        return 0
    )
    return 1
function main() returns nothing:
    optional[list[int64]] present = some(list(2, 3))
    optional[list[int64]] absent = none
    result[list[int64], string] success = ok(list(5, 7))
    result[list[int64], string] failure = fail("absent")
    int64 first = project_optional(view present)
    int64 second = project_optional(view absent)
    int64 third = project_result(view success)
    int64 fourth = project_result(view failure)
    int64 combined = first + second + third + fourth
    return nothing
"#;

fn lower(source: &str, release: bool) -> (Program, TypeInterner) {
    let file = FileId::new(0);
    let parsed = jett_parser::parse(source, file);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    let checked = jett_typecheck::check_with_options(
        &parsed.module,
        &resolved,
        jett_typecheck::CheckOptions { release },
    );
    assert!(
        checked
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != jett_diagnostics::Severity::Error),
        "{:?}",
        checked.diagnostics
    );
    let mut hir = jett_hir::lower(
        &parsed.module,
        &resolved,
        &checked,
        &HashMap::from([(file, SourceOrigin::Project)]),
    )
    .unwrap();
    jett_hir::complete_value_conversions(&mut hir, &checked.interner).unwrap();
    let mir = jett_mir::lower(&hir, &checked.interner).unwrap();
    (mir, checked.interner)
}

fn object(
    program: &Program,
    types: &TypeInterner,
    release: bool,
) -> Result<ObjectArtifact, CodegenError> {
    let entry = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "main")
        .unwrap()
        .id;
    crate::emit_host_program_object_with_options(
        program,
        types,
        entry,
        CodegenOptions { optimize: release },
    )
}

fn projection(program: &Program, name: &str) -> (FunctionId, BlockId, usize) {
    let function = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == name)
        .unwrap();
    for block in &function.blocks {
        for (index, statement) in block.statements.iter().enumerate() {
            if matches!(statement.kind, StatementKind::SumTake { success: true, .. })
                && function
                    .ordinary_borrowed_sum_projection(block.id, index)
                    .unwrap()
                    .is_some()
            {
                return (function.id, block.id, index);
            }
        }
    }
    panic!("genuine {name} has no projected success");
}

/// Public lowering constructs an ordinary empty seed. Copying editable fields
/// into it cannot mint the private original borrowed-Handle proof.
fn without_proof(original: &Function, release: bool) -> Function {
    let (mut seed, _) = lower(
        "namespace app\nfunction main() returns nothing:\n    return nothing\n",
        release,
    );
    let mut function = seed.functions.remove(0);
    function.id = original.id;
    function.identity = original.identity.clone();
    function.debug_kind = original.debug_kind.clone();
    function.params = original.params.clone();
    function.capture_count = original.capture_count;
    function.return_type = original.return_type;
    function.locals = original.locals.clone();
    function.entry = original.entry;
    function.blocks = original.blocks.clone();
    function.span = original.span;
    function
}

#[derive(Debug, Clone, Copy)]
enum Mutation {
    Copy,
    Retarget,
    Source,
    OutputOrigin,
    PayloadType,
    Guard,
    MissingProof,
}

#[test]
fn ordinary_borrowed_sum_public_object_rejects_forged_current_projections() {
    for release in [false, true] {
        let (baseline, types) = lower(SOURCE, release);
        let emitted = object(&baseline, &types, release).expect("genuine checked Source object");
        assert!(!emitted.bytes.is_empty());
        let (function, block, index) = projection(&baseline, "project_optional");
        for mutation in [
            Mutation::Copy,
            Mutation::Retarget,
            Mutation::Source,
            Mutation::OutputOrigin,
            Mutation::PayloadType,
            Mutation::Guard,
            Mutation::MissingProof,
        ] {
            let mut forged = baseline.clone();
            let function = &mut forged.functions[function.index() as usize];
            let row = function
                .ordinary_borrowed_sum_projection(block, index)
                .unwrap()
                .unwrap();
            let output = row.output();
            let tag = row.tag();
            match mutation {
                Mutation::Copy => {
                    let copy = function.blocks[block.index() as usize].statements[index].clone();
                    function.blocks[block.index() as usize]
                        .statements
                        .insert(index, copy);
                }
                Mutation::Retarget => {
                    let moved = function.blocks[block.index() as usize]
                        .statements
                        .remove(index);
                    function.blocks[function.entry.index() as usize]
                        .statements
                        .insert(0, moved);
                }
                Mutation::Source => {
                    let StatementKind::SumTake { source, .. } =
                        &mut function.blocks[block.index() as usize].statements[index].kind
                    else {
                        unreachable!()
                    };
                    *source = output;
                }
                Mutation::OutputOrigin => {
                    function.locals[output.index() as usize].view_source = None
                }
                Mutation::PayloadType => {
                    function.locals[output.index() as usize].ty = TypeInterner::INT64
                }
                Mutation::Guard => {
                    let branch = function.blocks.iter_mut().find(|block|
                        matches!(&block.terminator.kind, TerminatorKind::Branch { condition, .. }
                            if matches!(condition.kind, ExpressionKind::Local(local) if local == tag))).unwrap();
                    let TerminatorKind::Branch {
                        then_block,
                        else_block,
                        ..
                    } = &mut branch.terminator.kind
                    else {
                        unreachable!()
                    };
                    std::mem::swap(then_block, else_block);
                }
                Mutation::MissingProof => *function = without_proof(function, release),
            }
            match object(&forged, &types, release) {
                Err(CodegenError::InvalidMir(errors)) => {
                    assert!(!errors.is_empty(), "{mutation:?} release={release}")
                }
                result => panic!(
                    "{mutation:?} release={release}: public emission did not reject private-proof corruption: {result:?}"
                ),
            }
        }
    }
}

fn translated(
    program: &Program,
    types: &TypeInterner,
    name: &str,
) -> (ir::Function, HashMap<ir::FuncRef, String>) {
    let mut prepared = program.clone();
    jett_mir::prepare_native_sequences(&mut prepared, types);
    jett_mir::prepare_native_uninhabited_sums(&mut prepared, types);
    jett_mir::prepare_native_generated_functions(&mut prepared, types);
    let verified = verify_program(&prepared, types).unwrap();
    let flags = settings::Flags::new(settings::builder());
    let isa = isa::lookup(HOST).unwrap().finish(flags).unwrap();
    let mut module = ObjectModule::new(
        ObjectBuilder::new(
            isa,
            b"ordinary-projection-test".to_vec(),
            default_libcall_names(),
        )
        .unwrap(),
    );
    let declarations =
        declare_reachable_functions(&mut module, &prepared, types, &verified).unwrap();
    let function = prepared
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == name)
        .unwrap();
    let declaration = declarations.get(function.id).unwrap();
    let mut context = module.make_context();
    context.func.signature = declaration.signature.clone();
    translate_function(
        &mut module,
        &declarations,
        &prepared,
        function,
        types,
        &declaration.symbol,
        &mut context,
    )
    .unwrap();
    let names = context
        .func
        .dfg
        .ext_funcs
        .iter()
        .map(|(reference, data)| {
            let ir::ExternalName::User(user) = data.name else {
                panic!("unexpected import name")
            };
            let name = &context.func.params.user_named_funcs()[user];
            (
                reference,
                module
                    .declarations()
                    .get_function_decl(FuncId::from_u32(name.index))
                    .name
                    .clone()
                    .expect("declared runtime symbol"),
            )
        })
        .collect();
    (context.func, names)
}

fn calls(function: &ir::Function, names: &HashMap<ir::FuncRef, String>) -> Vec<(ir::Inst, String)> {
    function
        .layout
        .blocks()
        .flat_map(|block| function.layout.block_insts(block))
        .filter_map(|instruction| {
            let ir::InstructionData::Call { func_ref, .. } = function.dfg.insts[instruction] else {
                return None;
            };
            names
                .get(&func_ref)
                .cloned()
                .map(|name| (instruction, name))
        })
        .collect()
}

#[test]
fn ordinary_borrowed_sum_emission_keeps_success_borrowed_and_copies_only_failure_string() {
    for release in [false, true] {
        let (program, types) = lower(SOURCE, release);
        object(&program, &types, release).expect("checked public baseline");
        for (name, copies) in [("project_optional", 0), ("project_result", 1)] {
            let (function, names) = translated(&program, &types, name);
            let calls = calls(&function, &names);
            assert!(
                calls
                    .iter()
                    .any(|(_, name)| name == "jett_rt_v1_sum_handle_tag")
            );
            assert!(
                calls
                    .iter()
                    .any(|(_, name)| name == "jett_rt_v1_sum_payload_borrow")
            );
            assert_eq!(
                calls
                    .iter()
                    .filter(|(_, name)| name == NativeLeaf::Retain.symbol())
                    .count(),
                copies,
                "{name} release={release} calls={calls:?}\n{}",
                function.display()
            );
            assert!(
                !calls.iter().any(
                    |(_, name)| name == "jett_rt_v1_sum_take" || name == "jett_rt_v1_sum_clone"
                )
            );
            // The payload leaf consumes the borrowed reader's opaque bits and
            // never clears/stores its source slot; View formals have no slot.
            let takes = calls
                .iter()
                .filter(|(_, name)| name == "jett_rt_v1_sum_payload_borrow")
                .count();
            assert_eq!(takes, if copies == 0 { 1 } else { 2 });
        }
    }
}

#[test]
fn ordinary_borrowed_sum_scalar_width_and_pending_metadata_emit_without_owner_adoption() {
    for release in [false, true] {
        for (ty, literal, opcode) in [
            ("uint8", "7", ir::Opcode::Ireduce),
            ("int16", "7", ir::Opcode::Ireduce),
            ("float32", "1.5", ir::Opcode::Bitcast),
            ("float64", "1.5", ir::Opcode::Bitcast),
            ("int64", "7", ir::Opcode::Iconst),
        ] {
            let source = format!(
                "namespace app\nfunction payload(view source: optional[{ty}]) returns {ty}:\n    {ty} alias = view((view source) handle:\n        return {literal}\n    )\n    return alias\nfunction main() returns nothing:\n    {ty} value = {literal}\n    optional[{ty}] source = some(run run value)\n    {ty} read = payload(view source)\n    return nothing\n"
            );
            let (program, types) = lower(&source, release);
            object(&program, &types, release)
                .unwrap_or_else(|error| panic!("{ty} release={release}: {error:?}"));
            let (function, names) = translated(&program, &types, "payload");
            let calls = calls(&function, &names);
            assert!(
                calls
                    .iter()
                    .any(|(_, name)| name == "jett_rt_v1_sum_payload_pending_depth")
            );
            assert!(
                calls
                    .iter()
                    .any(|(_, name)| name == "jett_rt_v1_sum_payload_borrow")
            );
            assert!(
                !calls
                    .iter()
                    .any(|(_, name)| name == NativeLeaf::Retain.symbol()
                        || name == "jett_rt_v1_sum_take")
            );
            assert!(
                function
                    .layout
                    .blocks()
                    .flat_map(|block| function.layout.block_insts(block))
                    .any(|instruction| function.dfg.insts[instruction].opcode() == opcode)
            );
            assert_eq!(
                function.signature.returns.len(),
                2,
                "primitive pending depth remains in the actual ABI"
            );
            let depths = calls
                .iter()
                .filter(|(_, name)| name == "jett_rt_v1_sum_payload_pending_depth")
                .map(|(instruction, _)| function.dfg.inst_results(*instruction)[0])
                .collect::<Vec<_>>();
            assert!(
                function
                    .layout
                    .blocks()
                    .flat_map(|block| function.layout.block_insts(block))
                    .filter(|instruction| function.dfg.insts[*instruction].opcode()
                        == ir::Opcode::Return)
                    .any(
                        |instruction| function.dfg.inst_args(instruction).get(1).is_some_and(
                            |depth| depths
                                .iter()
                                .any(|original| function.dfg.resolve_aliases(*depth)
                                    == function.dfg.resolve_aliases(*original))
                        )
                    ),
                "{ty}: actual payload pending depth reaches the return; it is not reset to zero"
            );
        }
    }
}

const STRING_SUCCESS: &str = r#"namespace app
function copied_optional(view source: optional[string]) returns string:
    string alias = view((view source) handle:
        return "missing"
    )
    return alias
function copied_result(view source: result[string, string]) returns string:
    string alias = view((view source) handle error:
        return error
    )
    return alias
function main() returns nothing:
    optional[string] optional_source = some("optional")
    result[string, string] result_source = ok("result")
    string first = copied_optional(view optional_source)
    string second = copied_optional(view optional_source)
    string third = copied_result(view result_source)
    string fourth = copied_result(view result_source)
    return nothing
"#;

fn integer_constant(function: &ir::Function, value: Value) -> Option<i64> {
    let ir::ValueDef::Result(instruction, _) =
        function.dfg.value_def(function.dfg.resolve_aliases(value))
    else {
        return None;
    };
    match function.dfg.insts[instruction] {
        ir::InstructionData::UnaryImm {
            opcode: ir::Opcode::Iconst,
            imm,
            ..
        } => Some(imm.bits()),
        _ => None,
    }
}

#[test]
fn ordinary_borrowed_sum_string_success_copies_final_binding_once_from_nonowning_projection() {
    for release in [false, true] {
        let (program, types) = lower(STRING_SUCCESS, release);
        let emitted = object(&program, &types, release)
            .expect("genuine checked String-success public object");
        assert!(!emitted.bytes.is_empty());
        for name in ["copied_optional", "copied_result"] {
            let (id, block, index) = projection(&program, name);
            let source = &program.functions[id.index() as usize];
            let row = source
                .ordinary_borrowed_sum_projection(block, index)
                .unwrap()
                .unwrap();
            let final_alias = source.local(row.alias()).unwrap();
            assert_eq!(final_alias.ty, TypeInterner::STRING);
            assert_eq!(
                final_alias.view_source, None,
                "preserve the checked String copy header"
            );
            let storage = MoveValuePlan::analyze(&program, source, &types).unwrap();
            assert!(
                storage
                    .owned_locals
                    .contains(&(row.alias().index() as usize))
            );
            assert!(
                !storage
                    .owned_locals
                    .contains(&(row.output().index() as usize)),
                "projected temporary must remain nonowning"
            );
            assert_eq!(
                source.local(row.output()).unwrap().view_source,
                Some(row.backing())
            );

            let (function, names) = translated(&program, &types, name);
            let calls = calls(&function, &names);
            assert!(
                !calls.iter().any(
                    |(_, name)| name == "jett_rt_v1_sum_take" || name == "jett_rt_v1_sum_clone"
                )
            );
            let success = calls
                .iter()
                .filter(|(_, name)| name == "jett_rt_v1_sum_payload_borrow")
                .filter(|(instruction, _)| {
                    integer_constant(&function, function.dfg.inst_args(*instruction)[2]) == Some(1)
                })
                .map(|(instruction, _)| function.dfg.inst_results(*instruction)[0])
                .collect::<Vec<_>>();
            assert_eq!(success.len(), 1, "one guarded success projection");
            let payload = function.dfg.resolve_aliases(success[0]);
            let alias_copies = calls
                .iter()
                .filter(|(_, name)| name == NativeLeaf::Retain.symbol())
                .filter(|(instruction, _)| {
                    function
                        .dfg
                        .resolve_aliases(function.dfg.inst_args(*instruction)[1])
                        == payload
                })
                .count();
            assert_eq!(
                alias_copies,
                1,
                "the final String binding retains exactly once directly from borrowed bits; later Return copies are distinct; calls={calls:?}\n{}",
                function.display()
            );
        }
    }
}

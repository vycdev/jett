use super::*;
use cranelift_codegen::{dominator_tree::DominatorTree, flowgraph::ControlFlowGraph};
#[path = "returned_hook_order.rs"]
mod ordering;
const FACTORY: &str = include_str!(
    "../../../../../jett_driver/tests/native_conformance/resource/27_returned_hook_factory.jett"
);
const BORROW: &str = include_str!(
    "../../../../../jett_driver/tests/native_conformance/resource/28_returned_hook_borrow.jett"
);
const IMMEDIATE: &str = include_str!(
    "../../../../../jett_driver/tests/native_conformance/resource/29_immediate_returned_hook_close.jett"
);
const ORDER: &str = include_str!(
    "../../../../../jett_driver/tests/native_conformance/resource/30_immediate_returned_hook_factory.jett"
);
const CALLEE_FAILURE: &str = include_str!(
    "../../../../../jett_driver/tests/native_conformance/resource/31_immediate_returned_hook_callee_failure.jett"
);
const ACTUAL_FAILURE: &str = include_str!(
    "../../../../../jett_driver/tests/native_conformance/resource/32_immediate_returned_hook_actual_failure.jett"
);

const CLOSE: &str = include_str!(
    "../../../../../jett_driver/tests/native_conformance/resource/23_returned_hook_close.jett"
);
const ALIAS: &str = include_str!(
    "../../../../../jett_driver/tests/native_conformance/resource/24_returned_hook_alias.jett"
);
const RELAY: &str = include_str!(
    "../../../../../jett_driver/tests/native_conformance/resource/25_returned_hook_relay.jett"
);
const UNUSED: &str = include_str!(
    "../../../../../jett_driver/tests/native_conformance/resource/26_returned_hook_unused.jett"
);

fn hook_clif(
    source: &str,
    release: bool,
    selected_name: &str,
) -> (ir::Function, HashMap<ir::FuncRef, String>, Vec<u32>) {
    let checked = checked(source, release);
    let types = &checked.checked().interner;
    let mut program = lower_source(&checked);
    jett_mir::prepare_native_sequences(&mut program, types);
    jett_mir::prepare_native_uninhabited_sums(&mut program, types);
    jett_mir::prepare_native_generated_functions(&mut program, types);
    let layout = EmittedResourceLayout::from_program(&program, types, entry(&program)).unwrap();
    let verified = crate::verify::verify_resource_program(layout.plan()).unwrap();
    let isa = isa::lookup(HOST)
        .unwrap()
        .finish(settings::Flags::new(settings::builder()))
        .unwrap();
    let mut module = ObjectModule::new(
        ObjectBuilder::new(isa, "returned_hook_clif", default_libcall_names()).unwrap(),
    );
    let mut declarations =
        DeclaredFunctions::new(program.functions.len(), verified.functions().len());
    for item in verified.functions() {
        let function = program_function(&program, item.mir_id).unwrap();
        let signature = signature(&module, function, types, &layout).unwrap();
        let native_id = module
            .declare_function(&item.symbol, Linkage::Local, &signature)
            .unwrap();
        declarations
            .insert(DeclaredFunction {
                mir_id: function.id,
                native_id,
                symbol: item.symbol.clone(),
                signature,
                modes: function
                    .params
                    .iter()
                    .map(|parameter| parameter.mode)
                    .collect(),
                parameter_types: function
                    .params
                    .iter()
                    .map(|parameter| parameter.ty)
                    .collect(),
                debug_label: debug::function_label(function).unwrap(),
            })
            .unwrap();
    }
    let selected = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == selected_name)
        .unwrap();
    let declaration = declarations.get(selected.id).unwrap();
    let mut context = module.make_context();
    context.func.signature = declaration.signature.clone();
    translate_function_inner(
        &mut module,
        &declarations,
        &program,
        selected,
        types,
        &declaration.symbol,
        &mut context,
        Some(&layout),
    )
    .unwrap();
    context.verify(module.isa()).unwrap();
    let indirect = layout
        .plan()
        .function(selected.id)
        .unwrap()
        .operations()
        .iter()
        .filter(|operation| operation.indirect_hook_target().is_some())
        .map(|operation| layout.operation(selected.id, operation.id()).unwrap())
        .collect();
    let names = context
        .func
        .dfg
        .ext_funcs
        .iter()
        .map(|(reference, data)| {
            let ir::ExternalName::User(user) = data.name else {
                panic!("unexpected external name");
            };
            let name = &context.func.params.user_named_funcs()[user];
            let symbol = module
                .declarations()
                .get_function_decl(FuncId::from_u32(name.index))
                .name
                .clone()
                .expect("Source/runtime import has its declared symbol");
            let name = declarations
                .iter()
                .find(|declaration| declaration.symbol == symbol)
                .map(|declaration| {
                    program_function(&program, declaration.mir_id)
                        .unwrap()
                        .identity
                        .declaration
                        .name
                        .clone()
                })
                .unwrap_or(symbol);
            (reference, name)
        })
        .collect();
    (context.func, names, indirect)
}
fn instructions(function: &ir::Function) -> Vec<ir::Inst> {
    function
        .layout
        .blocks()
        .flat_map(|block| function.layout.block_insts(block))
        .collect()
}
fn call_name<'a>(
    function: &ir::Function,
    names: &'a HashMap<ir::FuncRef, String>,
    instruction: ir::Inst,
) -> Option<&'a str> {
    let ir::InstructionData::Call { func_ref, .. } = function.dfg.insts[instruction] else {
        return None;
    };
    names.get(&func_ref).map(String::as_str)
}
fn defining_instruction(function: &ir::Function, value: Value) -> ir::Inst {
    let ir::ValueDef::Result(instruction, _) =
        function.dfg.value_def(function.dfg.resolve_aliases(value))
    else {
        panic!("expected instruction result");
    };
    instruction
}
fn constant(function: &ir::Function, value: Value) -> Option<i64> {
    match function.dfg.insts[defining_instruction(function, value)] {
        ir::InstructionData::UnaryImm {
            opcode: ir::Opcode::Iconst,
            imm,
            ..
        } => Some(imm.bits()),
        _ => None,
    }
}
fn checked_branch(function: &ir::Function, status: Value) -> (ir::Block, ir::Block) {
    for instruction in instructions(function) {
        let ir::InstructionData::Brif { arg, blocks, .. } = function.dfg.insts[instruction] else {
            continue;
        };
        if let ir::InstructionData::IntCompareImm {
            cond,
            arg: checked,
            imm,
            ..
        } = function.dfg.insts[defining_instruction(function, arg)]
        {
            if checked == status && cond == IntCC::Equal && imm.bits() == 0 {
                return (
                    blocks[0].block(&function.dfg.value_lists),
                    blocks[1].block(&function.dfg.value_lists),
                );
            }
        }
    }
    panic!("exact status-zero output guard is absent");
}

#[test]
fn resource_returned_hook_unused_selector_retains_its_transitive_ordinary_helper_declaration() {
    for release in [false, true] {
        let checked = checked(CLOSE, release);
        let types = &checked.checked().interner;
        let mut program = lower_source(&checked);
        jett_mir::prepare_native_sequences(&mut program, types);
        jett_mir::prepare_native_uninhabited_sums(&mut program, types);
        jett_mir::prepare_native_generated_functions(&mut program, types);
        let selector = program
            .functions
            .iter()
            .find(|function| {
                function.identity.declaration.namespace == "resource_probe"
                    && function.identity.declaration.name == "select_create"
            })
            .unwrap();
        let helper = program
            .functions
            .iter()
            .find(|function| {
                function.identity.declaration.namespace == "resource_probe"
                    && function.identity.declaration.name == "terminal_label"
            })
            .unwrap();
        let ordinary =
            crate::reachability::reachable_function_ids_with_types(&program, types).unwrap();
        assert!(!ordinary.contains(&selector.id));
        assert!(!ordinary.contains(&helper.id));
        let layout = EmittedResourceLayout::from_program(&program, types, entry(&program)).unwrap();
        assert!(layout.plan().function(selector.id).is_some());
        assert!(layout.plan().function(helper.id).is_none());
        let verified = crate::verify::verify_resource_program(layout.plan()).unwrap();
        let helper_symbol = &verified
            .get(helper.id)
            .expect("an executable Resource root retains its ordinary helper declaration")
            .symbol;
        let (definitions, _) = symbols(&emitted(CLOSE, release));
        assert!(definitions.iter().any(|symbol| symbol == helper_symbol));

        let (function, names, indirect) = hook_clif(CLOSE, release, "select_create");
        assert!(indirect.is_empty());
        let calls = instructions(&function)
            .into_iter()
            .filter(|instruction| {
                call_name(&function, &names, *instruction) == Some("terminal_label")
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        // This pure helper has no Resource operation or hidden Scope operand.
        assert_eq!(function.dfg.inst_args(calls[0]).len(), 2);
    }
}

#[test]
fn resource_returned_hook_original_sources_use_opaque_metadata_without_ordinary_callable_ownership()
{
    for release in [false, true] {
        for source in [
            CLOSE,
            ALIAS,
            RELAY,
            UNUSED,
            FACTORY,
            BORROW,
            IMMEDIATE,
            ORDER,
            CALLEE_FAILURE,
            ACTUAL_FAILURE,
        ] {
            let artifact = emitted(source, release);
            let (_, imports) = symbols(&artifact);
            assert!(
                imports
                    .iter()
                    .any(|name| name == "jett_rt_v1_resource_descriptor")
            );
            assert!(
                imports
                    .iter()
                    .any(|name| name == "jett_rt_v1_resource_source_status")
            );
            for forbidden in [
                "jett_rt_v1_struct_clone",
                "jett_rt_v1_struct_drop",
                "jett_rt_v1_function_callable_check",
                "jett_rt_v1_struct_field",
            ] {
                assert!(
                    !imports.iter().any(|name| name == forbidden),
                    "{forbidden}: {imports:?}"
                );
            }
            let checked = checked(source, release);
            let program = lower_source(&checked);
            let plan = jett_mir::validate_resource_ownership(&program, &checked.checked().interner)
                .unwrap();
            for function in plan.functions() {
                if let Some(hook) = function.descriptor_return() {
                    assert!(
                        scalar_kind(
                            &checked.checked().interner,
                            hook.function_type(),
                            "public hook signature"
                        )
                        .is_err()
                    );
                    let companion =
                        jett_mir::ResourceCompanionPlan::analyze(&plan, function.function())
                            .unwrap();
                    for local in &program.functions[function.function().index() as usize].locals {
                        if function.descriptor_local(local.id).is_some() {
                            assert!(
                                !companion
                                    .storage()
                                    .owned_locals
                                    .contains(&(local.id.index() as usize))
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn resource_returned_hook_close_checks_descriptor_after_owner_stage_and_completed_source_before_effect()
 {
    for release in [false, true] {
        for source in [CLOSE, ALIAS, RELAY] {
            let (function, names, indirect) = hook_clif(source, release, "main");
            let all = instructions(&function);
            assert!(
                !all.iter()
                    .any(|instruction| function.dfg.insts[*instruction].opcode()
                        == ir::Opcode::CallIndirect)
            );
            let close = all
                .iter()
                .copied()
                .find(|instruction| {
                    call_name(&function, &names, *instruction)
                        == Some("jett_rt_v1_resource_descriptor_close")
                })
                .unwrap();
            let prepare = all
                .iter()
                .copied()
                .find(|instruction| {
                    call_name(&function, &names, *instruction)
                        == Some("jett_rt_v1_resource_hook_prepare")
                        && constant(&function, function.dfg.inst_args(*instruction)[3]) != Some(0)
                })
                .unwrap();
            assert_eq!(indirect.len(), 1);
            assert_eq!(
                constant(&function, function.dfg.inst_args(prepare)[2]),
                Some(i64::from(indirect[0]))
            );
            let stage = all
                .iter()
                .copied()
                .rfind(|instruction| {
                    call_name(&function, &names, *instruction)
                        == Some("jett_rt_v1_resource_transfer")
                })
                .unwrap();
            let complete = all
                .iter()
                .copied()
                .find(|instruction| {
                    call_name(&function, &names, *instruction)
                        == Some("jett_rt_v1_resource_source_status")
                })
                .unwrap();
            let mut cfg = ControlFlowGraph::new();
            cfg.compute(&function);
            let mut dom = DominatorTree::new();
            dom.compute(&function, &cfg);
            assert!(dom.dominates(stage, prepare, &function.layout));
            assert!(dom.dominates(complete, prepare, &function.layout));
            let status = function.dfg.inst_results(prepare)[0];
            let (accepted, refused) = checked_branch(&function, status);
            assert!(dom.dominates(accepted, close, &function.layout));
            let mut pending = vec![refused];
            let mut seen = std::collections::HashSet::new();
            while let Some(block) = pending.pop() {
                if seen.insert(block) {
                    assert_ne!(Some(block), function.layout.inst_block(close));
                    pending.extend(cfg.succ_iter(block));
                }
            }
            assert!(seen.iter().any(|block| {
                function.layout.block_insts(*block).any(|instruction| {
                    call_name(&function, &names, instruction)
                        == Some("jett_rt_v1_resource_scope_complete")
                })
            }));
        }
    }
}

#[test]
fn resource_returned_hook_scope_completion_guards_real_return_and_failure_cannot_publish_metadata()
{
    for release in [false, true] {
        for (source, selected) in [(CLOSE, "descriptor"), (RELAY, "relay")] {
            let (function, names, _) = hook_clif(source, release, selected);
            let all = instructions(&function);
            let complete = all
                .iter()
                .copied()
                .find(|instruction| {
                    call_name(&function, &names, *instruction)
                        == Some("jett_rt_v1_resource_scope_complete")
                })
                .unwrap();
            let body = function.dfg.inst_args(complete)[3];
            assert_eq!(constant(&function, body), Some(0));
            let (accepted, refused) =
                checked_branch(&function, function.dfg.inst_results(complete)[0]);
            let mut cfg = ControlFlowGraph::new();
            cfg.compute(&function);
            let mut dom = DominatorTree::new();
            dom.compute(&function, &cfg);
            let returned = all
                .iter()
                .copied()
                .find(|instruction| {
                    function.dfg.insts[*instruction].opcode() == ir::Opcode::Return
                        && dom.dominates(accepted, *instruction, &function.layout)
                })
                .unwrap();
            let value = function.dfg.inst_args(returned)[0];
            assert_ne!(constant(&function, value), Some(0));
            let mut pending = vec![refused];
            let mut seen = std::collections::HashSet::new();
            while let Some(block) = pending.pop() {
                if seen.insert(block) {
                    assert_ne!(Some(block), function.layout.inst_block(returned));
                    for instruction in function.layout.block_insts(block) {
                        if function.dfg.insts[instruction].opcode() == ir::Opcode::Return {
                            assert_eq!(
                                constant(&function, function.dfg.inst_args(instruction)[0]),
                                Some(0)
                            );
                        }
                    }
                    pending.extend(cfg.succ_iter(block));
                }
            }
            assert!(
                !all.iter()
                    .any(|instruction| call_name(&function, &names, *instruction)
                        == Some("jett_rt_v1_resource_return_publish"))
            );
        }
    }
}

use super::*;

const NAMED: &str = include_str!("../named_indirect/18_indirect_named_close.jett");
const WRONG: &str = include_str!("../named_indirect/19_indirect_named_terminal_failure.jett");
const ALIAS: &str = include_str!("../named_indirect/20_indirect_named_alias.jett");
const STDLIB: &str = r#"namespace app
export function main(net: Network) returns nothing:
    use resource_probe
    function(resource_probe.TestHandle) returns nothing dispose = resource_probe.close
    resource_probe.TestHandle token = resource_probe.create(view net, 701) handle error:
        return nothing
    dispose(token)
    return nothing
"#;

fn named_clif(source: &str, release: bool) -> (ir::Function, HashMap<ir::FuncRef, String>, String) {
    let checked = checked(source, release);
    let types = &checked.checked().interner;
    let mut program = lower_source(&checked);
    jett_mir::prepare_native_sequences(&mut program, types);
    jett_mir::prepare_native_uninhabited_sums(&mut program, types);
    jett_mir::prepare_native_generated_functions(&mut program, types);
    let selected = entry(&program);
    let layout = EmittedResourceLayout::from_program(&program, types, selected).unwrap();
    let verified = crate::verify::verify_resource_program(layout.plan()).unwrap();
    let isa = isa::lookup(HOST)
        .unwrap()
        .finish(settings::Flags::new(settings::builder()))
        .unwrap();
    let mut module = ObjectModule::new(
        ObjectBuilder::new(isa, "named_source_clif", default_libcall_names()).unwrap(),
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
    let plan = layout.plan().function(selected).unwrap();
    let target = plan
        .operations()
        .iter()
        .find_map(|operation| operation.named_indirect_target())
        .unwrap();
    let target_symbol = declarations.get(target.function()).unwrap().symbol.clone();
    let function = program_function(&program, selected).unwrap();
    let declaration = declarations.get(selected).unwrap();
    let mut context = module.make_context();
    context.func.signature = declaration.signature.clone();
    translate_function_inner(
        &mut module,
        &declarations,
        &program,
        function,
        types,
        &declaration.symbol,
        &mut context,
        Some(&layout),
    )
    .unwrap();
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
            (
                reference,
                module
                    .declarations()
                    .get_function_decl(FuncId::from_u32(name.index))
                    .name
                    .clone()
                    .expect("translated Source and runtime imports have declared symbols"),
            )
        })
        .collect();
    (context.func, names, target_symbol)
}

fn defining_instruction(function: &ir::Function, value: Value) -> ir::Inst {
    let ir::ValueDef::Result(instruction, _) =
        function.dfg.value_def(function.dfg.resolve_aliases(value))
    else {
        panic!("expected an instruction result");
    };
    instruction
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

fn constant(function: &ir::Function, value: Value) -> Option<i64> {
    let instruction = defining_instruction(function, value);
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
fn resource_named_indirect_original_source_emits_selected_bridge_and_retains_named_targets() {
    for release in [false, true] {
        for source in [NAMED, WRONG, ALIAS, STDLIB] {
            let artifact = emitted(source, release);
            let (_, imports) = symbols(&artifact);
            for required in [
                "jett_rt_v1_function_callable_check",
                "jett_rt_v1_struct_field",
                "jett_rt_v1_resource_source_enter",
                "jett_rt_v1_resource_source_status",
            ] {
                assert!(
                    imports.iter().any(|name| name == required),
                    "{required}: {imports:?}"
                );
            }
            let checked = checked(source, release);
            let program = lower_source(&checked);
            let plan = jett_mir::validate_resource_ownership(&program, &checked.checked().interner)
                .unwrap();
            let selected_body = plan
                .function(entry(&program))
                .unwrap()
                .operations()
                .iter()
                .find_map(|operation| operation.named_indirect_target())
                .unwrap();
            let target = selected_body.function();
            assert!(
                scalar_kind(
                    &checked.checked().interner,
                    selected_body.signature_type(),
                    "public Resource-bearing callable signature"
                )
                .is_err()
            );
            let reachable = crate::reachability::reachable_function_ids_with_types(
                &program,
                &checked.checked().interner,
            )
            .unwrap();
            assert!(reachable.contains(&target));
            if source == STDLIB {
                assert_eq!(
                    program.functions[target.index() as usize]
                        .identity
                        .declaration
                        .origin,
                    SourceOrigin::Stdlib
                );
            }
        }
    }
}

#[test]
fn resource_named_indirect_checks_original_actuals_then_code_and_environment_before_source_entry() {
    use cranelift_codegen::{dominator_tree::DominatorTree, flowgraph::ControlFlowGraph};
    for release in [false, true] {
        for source in [NAMED, WRONG, ALIAS, STDLIB] {
            let (function, names, target) = named_clif(source, release);
            let instructions = function
                .layout
                .blocks()
                .flat_map(|block| function.layout.block_insts(block))
                .collect::<Vec<_>>();
            assert!(
                !instructions
                    .iter()
                    .any(|instruction| function.dfg.insts[*instruction].opcode()
                        == ir::Opcode::CallIndirect)
            );
            let guard = instructions
                .iter()
                .copied()
                .find(|instruction| {
                    let ir::InstructionData::Brif { arg, .. } = function.dfg.insts[*instruction]
                    else {
                        return false;
                    };
                    function.dfg.insts[defining_instruction(&function, arg)].opcode()
                        == ir::Opcode::Band
                })
                .unwrap();
            let ir::InstructionData::Brif { arg, blocks, .. } = function.dfg.insts[guard] else {
                unreachable!();
            };
            let checks = function.dfg.inst_args(defining_instruction(&function, arg));
            let ir::InstructionData::IntCompare {
                cond, args: code, ..
            } = function.dfg.insts[defining_instruction(&function, checks[0])]
            else {
                panic!("missing code comparison");
            };
            assert_eq!(cond, IntCC::Equal);
            let ir::InstructionData::FuncAddr { func_ref, .. } =
                function.dfg.insts[defining_instruction(&function, code[1])]
            else {
                panic!("missing selected native address");
            };
            assert_eq!(names[&func_ref], target);
            assert_eq!(
                call_name(&function, &names, defining_instruction(&function, code[0])),
                Some("jett_rt_v1_struct_field")
            );
            let code_field = function
                .dfg
                .inst_args(defining_instruction(&function, code[0]));
            assert_eq!(
                constant(&function, code_field[2]),
                Some(NATIVE_FUNCTION_CODE_FIELD as i64)
            );
            let ir::InstructionData::IntCompareImm {
                cond,
                arg: environment,
                imm,
                ..
            } = function.dfg.insts[defining_instruction(&function, checks[1])]
            else {
                panic!("missing environment comparison");
            };
            assert_eq!(cond, IntCC::Equal);
            assert_eq!(imm.bits(), 0);
            assert_eq!(
                call_name(
                    &function,
                    &names,
                    defining_instruction(&function, environment)
                ),
                Some("jett_rt_v1_struct_field")
            );
            let environment_field = function
                .dfg
                .inst_args(defining_instruction(&function, environment));
            assert_eq!(
                constant(&function, environment_field[2]),
                Some(NATIVE_FUNCTION_ENVIRONMENT_FIELD as i64)
            );
            assert_eq!(
                code_field[1], environment_field[1],
                "code and environment must be read from the same evaluated descriptor"
            );
            let refused = blocks[1]
                .args(&function.dfg.value_lists)
                .collect::<Vec<_>>();
            let ir::BlockArg::Value(status) = refused[0] else {
                panic!("missing refusal status");
            };
            assert_eq!(constant(&function, status), Some(1));
            let source_entry = instructions
                .iter()
                .copied()
                .rfind(|instruction| {
                    call_name(&function, &names, *instruction)
                        == Some("jett_rt_v1_resource_source_enter")
                })
                .unwrap();
            let actual = instructions
                .iter()
                .copied()
                .rfind(|instruction| {
                    call_name(&function, &names, *instruction)
                        == Some("jett_rt_v1_resource_source_actual")
                })
                .unwrap();
            let callable = instructions
                .iter()
                .copied()
                .find(|instruction| {
                    call_name(&function, &names, *instruction)
                        == Some("jett_rt_v1_function_callable_check")
                })
                .unwrap();
            let mut cfg = ControlFlowGraph::new();
            cfg.compute(&function);
            let mut dominators = DominatorTree::new();
            dominators.compute(&function, &cfg);
            assert!(dominators.dominates(actual, callable, &function.layout));
            assert!(dominators.dominates(callable, guard, &function.layout));
            assert!(dominators.dominates(
                blocks[0].block(&function.dfg.value_lists),
                source_entry,
                &function.layout
            ));
            assert!(!dominators.dominates(
                blocks[1].block(&function.dfg.value_lists),
                source_entry,
                &function.layout
            ));
            let mut suffix = vec![blocks[1].block(&function.dfg.value_lists)];
            let mut refused_blocks = std::collections::HashSet::new();
            while let Some(block) = suffix.pop() {
                if refused_blocks.insert(block) {
                    assert_ne!(Some(block), function.layout.inst_block(source_entry));
                    suffix.extend(cfg.succ_iter(block));
                }
            }
            assert!(refused_blocks.iter().any(|block| {
                function.layout.block_insts(*block).any(|instruction| {
                    call_name(&function, &names, instruction)
                        == Some("jett_rt_v1_resource_scope_complete")
                })
            }));
            let selected_call = instructions
                .iter()
                .copied()
                .find(|instruction| {
                    call_name(&function, &names, *instruction) == Some(target.as_str())
                })
                .unwrap();
            assert!(dominators.dominates(source_entry, selected_call, &function.layout));
            let actuals = function.dfg.inst_args(selected_call);
            assert_eq!(
                actuals.len(),
                4,
                "context, empty environment, hidden Scope and owned actual"
            );
            assert_eq!(constant(&function, actuals[1]), Some(0));
            let body_status = instructions
                .iter()
                .copied()
                .rfind(|instruction| {
                    call_name(&function, &names, *instruction)
                        == Some("jett_rt_v1_resource_source_status")
                })
                .unwrap();
            assert!(dominators.dominates(selected_call, body_status, &function.layout));
        }
    }
}

#[test]
fn resource_named_indirect_refuses_same_signature_producer_swap_and_escaped_family_descriptor() {
    for release in [false, true] {
        let checked = checked(NAMED, release);
        let original = lower_source(&checked);
        let selected = entry(&original);
        let wrong = original
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "wrong_target")
            .unwrap()
            .id;
        let mut changed = original.clone();
        let producer = changed.functions[selected.index() as usize]
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.statements)
            .find_map(|statement| {
                let StatementKind::Let { value, .. } = &mut statement.kind else {
                    return None;
                };
                matches!(value.kind, ExpressionKind::FunctionRef(_)).then_some(value)
            })
            .unwrap();
        producer.kind = ExpressionKind::FunctionRef(wrong);
        assert!(matches!(
            super::super::super::emit_for_triple(
                &changed,
                &checked.checked().interner,
                HOST,
                Some(selected),
                CodegenOptions { optimize: release }
            ),
            Err(CodegenError::InvalidMir(_))
        ));
        let escaped = NAMED.replace("export function main", "function retain(view callback: function(resource_probe.TestHandle) returns nothing) returns nothing:\n    return nothing\nexport function main")
            .replace("    dispose(token)", "    retain(view dispose)\n    dispose(token)");
        let escaped_checked = super::checked(&escaped, release);
        let escaped_program = lower_source(&escaped_checked);
        assert!(
            super::super::super::emit_for_triple(
                &escaped_program,
                &escaped_checked.checked().interner,
                HOST,
                Some(entry(&escaped_program)),
                CodegenOptions { optimize: release }
            )
            .is_err()
        );
    }
}

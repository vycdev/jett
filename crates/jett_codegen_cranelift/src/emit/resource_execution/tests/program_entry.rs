//! Actual generated-entry IR/object controls; no linked native execution.
use super::*;
use object::{Object, ObjectSection, ObjectSymbol, RelocationTarget, SymbolKind};
use std::collections::HashSet;

const BEGIN: &str = "jett_rt_v1_resource_program_begin";
const COMPLETE: &str = "jett_rt_v1_resource_program_complete";
const MANIFEST: &str = "jett_aot_resource_v1_manifest";

fn actual_entry(release: bool) -> (ir::Function, HashMap<ir::FuncRef, String>) {
    let checked = checked(SOURCE, release);
    let program = lower_source(&checked);
    let types = &checked.checked().interner;
    let layout = EmittedResourceLayout::from_program(&program, types, entry(&program)).unwrap();
    let verified = crate::verify::verify_resource_program(layout.plan()).unwrap();
    let target = isa::lookup(HOST)
        .unwrap()
        .finish(settings::Flags::new(settings::builder()))
        .unwrap();
    let mut module = ObjectModule::new(
        ObjectBuilder::new(target, "actual_resource_entry", default_libcall_names()).unwrap(),
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
    let (_, context) = build_entry(&mut module, &declarations, &layout).unwrap();
    context.verify(module.isa()).unwrap();
    let names = context
        .func
        .dfg
        .ext_funcs
        .iter()
        .map(|(reference, data)| {
            let ir::ExternalName::User(user) = data.name else {
                panic!("declared external function")
            };
            let name = &context.func.params.user_named_funcs()[user];
            let symbol = module
                .declarations()
                .get_function_decl(FuncId::from_u32(name.index))
                .name
                .clone()
                .unwrap();
            (reference, symbol)
        })
        .collect();
    (context.func, names)
}

fn call(function: &ir::Function, names: &HashMap<ir::FuncRef, String>, symbol: &str) -> ir::Inst {
    let calls = function.layout.blocks().flat_map(|block| function.layout.block_insts(block))
        .filter(|instruction| matches!(function.dfg.insts[*instruction],
            ir::InstructionData::Call { func_ref, .. } if names.get(&func_ref).is_some_and(|name| name == symbol)))
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1, "one actual call to {symbol}");
    calls[0]
}

fn destinations(function: &ir::Function, block: ir::Block) -> Vec<ir::Block> {
    let terminal = function.layout.last_inst(block).unwrap();
    function.dfg.insts[terminal]
        .branch_destination(&function.dfg.jump_tables, &function.dfg.exception_tables)
        .into_iter()
        .map(|branch| branch.block(&function.dfg.value_lists))
        .collect()
}

fn reachable_returns(function: &ir::Function, start: ir::Block) -> HashSet<ir::Block> {
    let mut pending = vec![start];
    let mut visited = HashSet::new();
    let mut returns = HashSet::new();
    while let Some(block) = pending.pop() {
        if !visited.insert(block) {
            continue;
        }
        let terminal = function.layout.last_inst(block).unwrap();
        if function.dfg.insts[terminal].opcode() == ir::Opcode::Return {
            returns.insert(block);
        } else {
            pending.extend(destinations(function, block));
        }
    }
    returns
}

fn returned(function: &ir::Function, block: ir::Block) -> &[Value] {
    function
        .dfg
        .inst_args(function.layout.last_inst(block).unwrap())
}

#[test]
fn generated_entry_uses_own_manifest_exact_context_and_separate_startup_outputs() {
    for release in [false, true] {
        let (function, names) = actual_entry(release);
        assert_eq!(function.signature.params, [AbiParam::new(ir::types::I64)]);
        assert_eq!(function.signature.returns, [AbiParam::new(ir::types::I32)]);
        let incoming = function
            .dfg
            .block_params(function.layout.entry_block().unwrap())[0];
        let manifest = call(&function, &names, MANIFEST);
        let begin = call(&function, &names, BEGIN);
        let complete = call(&function, &names, COMPLETE);
        let scope = call(&function, &names, "jett_rt_v1_resource_entry_scope");
        let manifest_args = function.dfg.inst_args(manifest);
        let begin_args = function.dfg.inst_args(begin);
        assert_eq!(manifest_args, [begin_args[1]]);
        assert_eq!(begin_args.len(), 4);
        assert_eq!(begin_args[0], incoming);
        assert_eq!(function.dfg.inst_args(scope)[0], incoming);
        assert_eq!(function.dfg.inst_args(complete)[0], incoming);
        assert_eq!(
            begin_args[1..]
                .iter()
                .copied()
                .collect::<HashSet<_>>()
                .len(),
            3
        );
        for (instruction, expected) in [
            (manifest, vec![ir::types::I64]),
            (begin, vec![ir::types::I64; 4]),
            (
                complete,
                vec![ir::types::I64, ir::types::I64, ir::types::I32],
            ),
        ] {
            let ir::InstructionData::Call { func_ref, .. } = function.dfg.insts[instruction] else {
                unreachable!()
            };
            let signature = &function.dfg.signatures[function.dfg.ext_funcs[func_ref].signature];
            assert_eq!(
                signature
                    .params
                    .iter()
                    .map(|parameter| parameter.value_type)
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!(signature.returns, [AbiParam::new(ir::types::I32)]);
        }
        let manifest_block = function.layout.inst_block(manifest).unwrap();
        let begin_block = function.layout.inst_block(begin).unwrap();
        let scope_block = function.layout.inst_block(scope).unwrap();
        assert_eq!(destinations(&function, manifest_block)[0], begin_block);
        assert_eq!(destinations(&function, begin_block)[0], scope_block);
        assert_eq!(
            destinations(&function, manifest_block)[1],
            destinations(&function, begin_block)[1]
        );
    }
}

#[test]
fn every_post_begin_return_uses_the_single_finish_gate_and_exact_terminal_status() {
    for release in [false, true] {
        let (function, names) = actual_entry(release);
        let begin = call(&function, &names, BEGIN);
        let complete = call(&function, &names, COMPLETE);
        let begin_block = function.layout.inst_block(begin).unwrap();
        let generated = function.layout.inst_block(complete).unwrap();
        assert_eq!(
            returned(&function, generated),
            function.dfg.inst_results(complete)
        );
        let gates = function
            .layout
            .blocks()
            .filter(|block| destinations(&function, *block).contains(&generated))
            .collect::<Vec<_>>();
        assert_eq!(gates.len(), 1);
        let finish = gates[0];
        let successors = destinations(&function, finish);
        assert_eq!(successors.len(), 2);
        assert_eq!(successors[1], generated);
        let borrowed = successors[0];
        assert_eq!(
            returned(&function, borrowed),
            &[function.dfg.block_params(finish)[0]]
        );
        assert_eq!(
            function.dfg.inst_args(complete)[2],
            function.dfg.block_params(finish)[0]
        );
        let terminal = function.layout.last_inst(finish).unwrap();
        let condition = function.dfg.inst_args(terminal)[0];
        let ir::ValueDef::Result(compare, _) = function.dfg.value_def(condition) else {
            panic!("borrowed sentinel test")
        };
        let ir::InstructionData::IntCompareImm { cond, arg, imm, .. } = function.dfg.insts[compare]
        else {
            panic!("exact zero sentinel comparison")
        };
        assert_eq!(cond, IntCC::Equal);
        assert_eq!(i64::from(imm), 0);
        assert_eq!(arg, function.dfg.inst_args(complete)[1]);
        let begin_success = destinations(&function, begin_block)[0];
        assert_eq!(
            reachable_returns(&function, begin_success),
            HashSet::from([borrowed, generated])
        );
        let startup_refusal = destinations(&function, begin_block)[1];
        assert_eq!(
            reachable_returns(&function, startup_refusal),
            HashSet::from([startup_refusal])
        );
        assert_eq!(
            function
                .layout
                .blocks()
                .filter(
                    |block| function.dfg.insts[function.layout.last_inst(*block).unwrap()].opcode()
                        == ir::Opcode::Return
                )
                .count(),
            3
        );
    }
}

fn relocation_callers(artifact: &ObjectArtifact, target_name: &str) -> Vec<String> {
    let file = object::File::parse(artifact.bytes.as_slice()).unwrap();
    let target = file
        .symbols()
        .find(|symbol| symbol.name().ok() == Some(target_name))
        .unwrap()
        .index();
    let mut callers = Vec::new();
    for section in file.sections() {
        for (offset, relocation) in section.relocations() {
            if relocation.target() != RelocationTarget::Symbol(target) {
                continue;
            }
            let address = section.address().checked_add(offset).unwrap();
            let owner = file
                .symbols()
                .filter(|symbol| {
                    symbol.is_definition()
                        && symbol.kind() == SymbolKind::Text
                        && symbol.section_index() == Some(section.index())
                        && symbol.address() <= address
                        && (symbol.size() == 0
                            || address < symbol.address().checked_add(symbol.size()).unwrap())
                })
                .max_by_key(|symbol| symbol.address())
                .unwrap();
            callers.push(owner.name().unwrap().to_owned());
        }
    }
    callers
}

#[test]
fn real_resource_object_links_startup_only_from_its_generated_c_entry() {
    for release in [false, true] {
        let artifact = emitted(SOURCE, release);
        let (definitions, imports) = symbols(&artifact);
        assert_eq!(
            definitions
                .iter()
                .filter(|name| name.as_str() == MANIFEST)
                .count(),
            1
        );
        assert!(!imports.iter().any(|name| name == MANIFEST));
        for target in [BEGIN, COMPLETE, MANIFEST] {
            assert_eq!(
                relocation_callers(&artifact, target),
                [JETT_AOT_ENTRY_SYMBOL_V1.to_owned()]
            );
        }
        for observer in ["unwind", "failure", "failure_copy"] {
            assert!(
                !imports
                    .iter()
                    .any(|name| name == &format!("jett_rt_v1_resource_program_{observer}"))
            );
        }
    }
}

#[test]
fn ordinary_objects_keep_the_same_entry_without_resource_startup_or_manifest() {
    const ORDINARY: &str =
        "namespace app\nexport function main() returns nothing:\n    return nothing\n";
    for release in [false, true] {
        let file = FileId::new(0);
        let parsed = parse(ORDINARY, file);
        assert!(parsed.errors.is_empty());
        let resolved = jett_resolve::resolve(&parsed.module);
        let checked = jett_typecheck::check(&parsed.module, &resolved);
        assert!(
            checked
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.severity != jett_diagnostics::Severity::Error)
        );
        let hir = jett_hir::lower(
            &parsed.module,
            &resolved,
            &checked,
            &HashMap::from([(file, SourceOrigin::Project)]),
        )
        .unwrap();
        let program = jett_mir::lower(&hir, &checked.interner).unwrap();
        let artifact = super::super::super::emit_for_triple(
            &program,
            &checked.interner,
            HOST,
            Some(entry(&program)),
            CodegenOptions { optimize: release },
        )
        .unwrap();
        let (definitions, imports) = symbols(&artifact);
        assert_eq!(
            definitions
                .iter()
                .filter(|name| name.as_str() == JETT_AOT_ENTRY_SYMBOL_V1)
                .count(),
            1
        );
        assert!(
            !definitions
                .iter()
                .chain(&imports)
                .any(|name| name == MANIFEST || name.starts_with("jett_rt_v1_resource_"))
        );
    }
}

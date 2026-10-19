use super::*;
use jett_comptime::checked_types::CheckedExpressionTypes;
use object::{Object, ObjectSection, ObjectSymbol, RelocationTarget, SymbolKind};

include!("carrier_wrapper_sources.rs");

const WHOLE_SOURCE: &str = include_str!(
    "../../../../../jett_mir/src/resource_ownership/fixtures/17_absent_aggregate_shapes.jett"
);
const MINIMAL_SUPPORT: &str = "namespace resource_probe\nexport resource TestHandle\n";
const PURE: &str = r#"namespace app
function helper() returns int64:
    return 7
export function main(net: Network) returns nothing:
    int64 answer = helper()
    if answer != 7:
        return nothing
    return nothing
"#;
const CALLEE: &str = r#"namespace app
function helper() returns int64:
    return 7
export function main() returns nothing:
    list[int64] values = list(1, 2)
    mutable int64 total = 0
    for number in values:
        total = total + number
    if total != 3:
        int64 ignored = helper()
    return nothing
function reenter() returns nothing:
    main()
    return nothing
"#;

fn named<'a>(program: &'a Program, name: &str) -> &'a Function {
    program
        .functions
        .iter()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == name
        })
        .unwrap_or_else(|| panic!("checked app.{name}"))
}

fn checked_support(source: &str, support: &str, release: bool) -> Arc<CheckedResourceProgram> {
    let stdlib = FileId::new(10_000);
    let project = FileId::new(0);
    let mut parsed = parse(support, stdlib);
    let primary = parse(source, project);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert!(primary.errors.is_empty(), "{:?}", primary.errors);
    let declaration = parsed
        .module
        .items
        .iter()
        .find_map(|item| match item {
            Item::Resource(resource) => Some(resource.name.span),
            _ => None,
        })
        .unwrap();
    parsed.module.items.extend(primary.module.items);
    let catalog = [
        ("kernel_create", ResourceKernelRecipe::NetworkFactory),
        ("kernel_borrow", ResourceKernelRecipe::NetworkBorrow),
        ("kernel_close", ResourceKernelRecipe::Finalize),
    ]
    .into_iter()
    .map(|(member, recipe)| ResourceKernelSpec {
        resource_declaration: declaration,
        member: member.into(),
        recipe,
    })
    .collect::<Vec<_>>();
    Arc::new(
        CheckedResourceProgram::prepare(
            parsed,
            HashMap::from([
                (stdlib, SourceOrigin::Stdlib),
                (project, SourceOrigin::Project),
            ]),
            &catalog,
            CheckOptions { release },
        )
        .unwrap_or_else(|error| panic!("{error:?}; {:?}", error.diagnostics())),
    )
}

fn materialized(checked: &Arc<CheckedResourceProgram>) -> Program {
    let expression_types = Arc::new(CheckedExpressionTypes {
        resource_program: Some(checked.clone()),
        expressions: checked
            .checked()
            .type_map
            .iter()
            .map(|(span, ty)| (*span, checked.checked().interner.type_name(*ty)))
            .collect(),
        ..Default::default()
    });
    let required = jett_comptime::evaluate_explicit_comptime_expressions_capture(
        checked.module(),
        Arc::new(jett_types::ReflectionMetadata::new()),
        expression_types,
        Arc::new(HashMap::new()),
    );
    assert!(
        required.diagnostics.is_empty(),
        "{:?}",
        required.diagnostics
    );
    assert!(required.debug_events.is_empty());
    required.values.checked_required_values(checked).unwrap();
    assert!(!required.values.is_empty());
    let types = &checked.checked().interner;
    let mut hir = jett_hir::lower_checked_resource_program(checked).unwrap();
    jett_hir::materialize_checked_required_values(&mut hir, &required.values, types).unwrap();
    jett_hir::complete_value_conversions(&mut hir, types).unwrap();
    jett_mir::lower(&hir, types).unwrap()
}

fn prepare(program: &mut Program, types: &TypeInterner) {
    jett_mir::prepare_native_sequences(program, types);
    jett_mir::prepare_native_uninhabited_sums(program, types);
    jett_mir::prepare_native_generated_functions(program, types);
}

fn ordinary_clif(
    program: &Program,
    types: &TypeInterner,
    selected: FunctionId,
    name: &str,
) -> (ir::Function, HashMap<ir::FuncRef, String>) {
    let layout = EmittedResourceLayout::from_program(program, types, selected).unwrap();
    let verified = crate::verify::verify_resource_program(layout.plan()).unwrap();
    let isa = isa::lookup(HOST)
        .unwrap()
        .finish(settings::Flags::new(settings::builder()))
        .unwrap();
    let mut module = ObjectModule::new(
        ObjectBuilder::new(isa, "ordinary_entry_clif", default_libcall_names()).unwrap(),
    );
    let mut declarations =
        DeclaredFunctions::new(program.functions.len(), verified.functions().len());
    for item in verified.functions() {
        let function = program_function(program, item.mir_id).unwrap();
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
    let function = named(program, name);
    assert!(layout.plan().function(function.id).is_none());
    let declaration = declarations.get(function.id).unwrap();
    let mut context = module.make_context();
    context.func.signature = declaration.signature.clone();
    let selected_layout = layout
        .plan()
        .function(function.id)
        .is_some()
        .then_some(&layout);
    assert!(
        selected_layout.is_none(),
        "ordinary body cannot select ResourceEmission"
    );
    translate_function_inner(
        &mut module,
        &declarations,
        program,
        function,
        types,
        &declaration.symbol,
        &mut context,
        selected_layout,
    )
    .unwrap();
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

fn calls_to(
    function: &ir::Function,
    names: &HashMap<ir::FuncRef, String>,
    symbol: &str,
) -> Vec<ir::Inst> {
    function
        .layout
        .blocks()
        .flat_map(|block| function.layout.block_insts(block))
        .filter(|instruction| {
            matches!(function.dfg.insts[*instruction], ir::InstructionData::Call { func_ref, .. }
            if names.get(&func_ref).is_some_and(|name| name == symbol))
        })
        .collect()
}

fn assert_ordinary_function(
    program: &Program,
    types: &TypeInterner,
    selected: FunctionId,
    name: &str,
) -> (ir::Function, HashMap<ir::FuncRef, String>) {
    let function = named(program, name);
    let (clif, names) = ordinary_clif(program, types, selected, name);
    assert_eq!(clif.signature.params.len(), function.params.len() + 2);
    assert!(
        clif.signature
            .params
            .iter()
            .all(|parameter| parameter.value_type == ir::types::I64)
    );
    assert!(
        !names
            .values()
            .any(|symbol| symbol.starts_with("jett_rt_v1_resource_")),
        "ordinary body imports Resource leaves: {names:?}"
    );
    let storage = MoveValuePlan::analyze(program, function, types).unwrap();
    if !function
        .locals
        .iter()
        .any(|local| jett_mir::move_values::is_linear(types, local.ty))
    {
        let copy = jett_mir::copy_values::CopyValuePlan::analyze(function, types).unwrap();
        assert_eq!(copy.owned_locals, storage.owned_locals);
    }
    for local in function.locals.iter().filter(|local| {
        matches!(types.resolve(local.ty), Type::List(_))
            && local.view_source.is_none()
            && !function.is_view_local(local.id)
    }) {
        assert!(
            storage.owned_locals.contains(&(local.id.index() as usize)),
            "ordinary consuming-list storage must own its actual holder"
        );
    }
    jett_mir::validate_call_ownership(program, types).unwrap();
    (clif, names)
}

fn import_callers(artifact: &ObjectArtifact, import: &str) -> Vec<String> {
    let file = object::File::parse(artifact.bytes.as_slice()).unwrap();
    let target = file
        .symbols()
        .find(|symbol| symbol.is_undefined() && symbol.name().ok() == Some(import))
        .expect("exact imported leaf")
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
                .expect("relocation belongs to an exact defined text symbol");
            callers.push(owner.name().unwrap().to_owned());
        }
    }
    callers
}

fn assert_wrapper_only_bridge(artifact: &ObjectArtifact) {
    let (definitions, imports) = symbols(artifact);
    assert!(
        definitions
            .iter()
            .any(|symbol| symbol == JETT_AOT_ENTRY_SYMBOL_V1)
    );
    assert!(
        definitions
            .iter()
            .any(|symbol| symbol == "jett_aot_resource_v1_manifest")
    );
    for leaf in [
        "entry_scope",
        "scope_validate",
        "scope_complete",
        "entry_outcome",
    ] {
        let leaf = format!("jett_rt_v1_resource_{leaf}");
        assert_eq!(
            imports
                .iter()
                .filter(|symbol| symbol.as_str() == leaf)
                .count(),
            1
        );
        assert_eq!(
            import_callers(artifact, &leaf),
            [JETT_AOT_ENTRY_SYMBOL_V1.to_owned()]
        );
    }
    assert!(
        imports
            .iter()
            .any(|symbol| symbol == NativeLeaf::Status.symbol())
    );
    for leaf in [
        "source_enter",
        "source_parameter",
        "source_status",
        "source_actual",
        "source_result",
        "return_publish",
    ] {
        assert!(
            !imports
                .iter()
                .any(|symbol| symbol == &format!("jett_rt_v1_resource_{leaf}"))
        );
    }
}

#[test]
fn ordinary_entry_genuine_pure_main_emits_only_wrapper_scope_with_two_hidden_body_parameters() {
    for release in [false, true] {
        let checked = checked_support(PURE, MINIMAL_SUPPORT, release);
        let types = &checked.checked().interner;
        let hir = jett_hir::lower_checked_resource_program(&checked).unwrap();
        assert!(hir.resource_source.required_materializations().is_empty());
        assert_eq!(hir.resource_manifest.hooks().count(), 3);
        let baseline = jett_mir::lower(&hir, types).unwrap();
        let selected = entry(&baseline);
        let mut prepared = baseline.clone();
        prepare(&mut prepared, types);
        let layout = EmittedResourceLayout::from_program(&prepared, types, selected).unwrap();
        let bridge = layout.plan().entry_scope().unwrap();
        assert_eq!(bridge.function(), selected);
        assert!(layout.entry_bridge_completion().is_some());
        assert!(layout.function_signature(selected).is_none());
        assert!(layout.frame(selected, bridge.root_scope().id()).is_none());
        assert!(
            layout
                .operation(selected, bridge.completion().id())
                .is_none()
        );
        let (main, names) = assert_ordinary_function(&prepared, types, selected, "main");
        let helper_symbol =
            crate::symbol_name(&named(&prepared, "helper").identity, types).unwrap();
        let calls = calls_to(&main, &names, &helper_symbol);
        assert_eq!(calls.len(), 1);
        assert_eq!(main.dfg.inst_args(calls[0]).len(), 2);
        assert_ordinary_function(&prepared, types, selected, "helper");
        let artifact = super::super::super::emit_for_triple(
            &baseline,
            types,
            HOST,
            Some(selected),
            CodegenOptions { optimize: release },
        )
        .unwrap();
        assert_wrapper_only_bridge(&artifact);
        assert_eq!(
            import_callers(&artifact, "jett_rt_v1_resource_entry_network"),
            [JETT_AOT_ENTRY_SYMBOL_V1.to_owned()]
        );
    }
}

#[test]
fn ordinary_entry_main_as_callee_and_consuming_for_keep_the_ordinary_abi() {
    for release in [false, true] {
        let checked = checked_support(CALLEE, MINIMAL_SUPPORT, release);
        let types = &checked.checked().interner;
        let baseline = lower_source(&checked);
        let selected = entry(&baseline);
        assert!(named(&baseline, "main").blocks.iter().any(|block| matches!(
            block.terminator.kind,
            jett_mir::TerminatorKind::ForEach { .. }
        )));
        let mut prepared = baseline.clone();
        prepare(&mut prepared, types);
        assert!(
            !named(&prepared, "main").blocks.iter().any(|block| matches!(
                block.terminator.kind,
                jett_mir::TerminatorKind::ForEach { .. }
            ))
        );
        assert_ordinary_function(&prepared, types, selected, "main");
        assert!(
            named(&prepared, "main")
                .blocks
                .iter()
                .flat_map(|block| &block.statements)
                .any(|statement| {
                    matches!(
                        statement.kind,
                        jett_mir::StatementKind::SequenceGet { consume: true, .. }
                    )
                }),
            "the genuine For must retain its consuming element extraction"
        );
        let (caller, names) = assert_ordinary_function(&prepared, types, selected, "reenter");
        let main_symbol = crate::symbol_name(&named(&prepared, "main").identity, types).unwrap();
        let calls = calls_to(&caller, &names, &main_symbol);
        assert_eq!(calls.len(), 1);
        assert_eq!(caller.dfg.inst_args(calls[0]).len(), 2);
        let artifact = super::super::super::emit_for_triple(
            &baseline,
            types,
            HOST,
            Some(selected),
            CodegenOptions { optimize: release },
        )
        .unwrap();
        assert_wrapper_only_bridge(&artifact);
        let (_, imports) = symbols(&artifact);
        assert!(
            !imports
                .iter()
                .any(|symbol| symbol == "jett_rt_v1_resource_entry_network")
        );
    }
}

#[test]
fn ordinary_entry_complete_source17_wrapper14_emits_main_and_required_helper_with_ordinary_signatures()
 {
    let (name, suffix) = WRAPPERS[13];
    assert_eq!(name, "14_required_primitive_controls");
    let source = format!("{WHOLE_SOURCE}{suffix}");
    for release in [false, true] {
        let checked = checked_support(&source, SUPPORT, release);
        let types = &checked.checked().interner;
        let baseline = materialized(&checked);
        let selected = entry(&baseline);
        let mut prepared = baseline.clone();
        prepare(&mut prepared, types);
        let (main, names) = assert_ordinary_function(&prepared, types, selected, "main");
        let helper_symbol = crate::symbol_name(
            &named(&prepared, "absent_required_controls").identity,
            types,
        )
        .unwrap();
        let calls = calls_to(&main, &names, &helper_symbol);
        assert_eq!(calls.len(), 1);
        assert_eq!(main.dfg.inst_args(calls[0]).len(), 2);
        assert_ordinary_function(&prepared, types, selected, "absent_required_controls");
        let artifact = super::super::super::emit_for_triple(
            &baseline,
            types,
            HOST,
            Some(selected),
            CodegenOptions { optimize: release },
        )
        .unwrap();
        let (definitions, imports) = symbols(&artifact);
        assert!(
            definitions
                .iter()
                .any(|symbol| symbol == JETT_AOT_ENTRY_SYMBOL_V1)
        );
        assert_eq!(
            imports
                .iter()
                .filter(|symbol| symbol.as_str() == "jett_rt_v1_resource_scope_complete")
                .count(),
            1
        );
        let callers = import_callers(&artifact, "jett_rt_v1_resource_scope_complete");
        assert_eq!(
            callers
                .iter()
                .filter(|symbol| symbol.as_str() == JETT_AOT_ENTRY_SYMBOL_V1)
                .count(),
            1
        );
        for name in ["main", "absent_required_controls"] {
            let symbol = crate::symbol_name(&named(&prepared, name).identity, types).unwrap();
            assert!(!callers.contains(&symbol));
        }
    }
}

#[test]
fn ordinary_entry_current_header_body_and_manifest_mutations_refuse_real_object_emission() {
    for release in [false, true] {
        let checked = checked_support(PURE, MINIMAL_SUPPORT, release);
        let types = &checked.checked().interner;
        let baseline = lower_source(&checked);
        let selected = entry(&baseline);
        for mutation in 0..4 {
            let mut changed = baseline.clone();
            let index = usize::try_from(selected.index()).unwrap();
            match mutation {
                0 => changed.functions[index].params[0].mode = jett_mir::ParamMode::View,
                1 => changed.functions[index].blocks[0].statements.clear(),
                2 => changed.functions[index].capture_count = 1,
                3 => changed.resource_manifest = jett_hir::ResourceManifest::empty(),
                _ => unreachable!(),
            }
            assert!(
                super::super::super::emit_for_triple(
                    &changed,
                    types,
                    HOST,
                    Some(selected),
                    CodegenOptions { optimize: release },
                )
                .is_err(),
                "mutation={mutation}; release={release}"
            );
        }
    }
}

use jett_common::{FileId, SourceOrigin};
use jett_parser::{ast::Item, parse};
use jett_resolve::ResourceKernelSpec;
use jett_typecheck::{CheckOptions, CheckedResourceProgram};
use jett_types::ResourceKernelRecipe;
use std::{collections::HashMap, sync::Arc};
const SUPPORT: &str =
    include_str!("../../jett_comptime/src/resource_execution/fixtures/resource_probe.jett");
const SOURCE: &str = include_str!("actor_graph/fixtures/07_nested_hook_actor.jett");
fn observed(source: &str, release: bool) -> serde_json::Value {
    let project = FileId::new(0);
    let support = FileId::new(10_000);
    let mut parsed = parse(SUPPORT, support);
    let primary = parse(source, project);
    let entry = primary
        .module
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name.name == "main" => Some(function.span),
            _ => None,
        })
        .expect("actual Source main");
    let resource = parsed
        .module
        .items
        .iter()
        .find_map(|item| match item {
            Item::Resource(value) => Some(value.name.span),
            _ => None,
        })
        .expect("unchanged actual Support resource");
    parsed.errors.extend(primary.errors);
    parsed.module.items.extend(primary.module.items);
    let mut out = serde_json::json!({"release_source":release,"desired_oracle":false,"parse_diagnostics":format!("{:?}",parsed.errors)});
    let catalog = [
        ("kernel_create", ResourceKernelRecipe::NetworkFactory),
        ("kernel_borrow", ResourceKernelRecipe::NetworkBorrow),
        ("kernel_close", ResourceKernelRecipe::Finalize),
    ]
    .into_iter()
    .map(|(member, recipe)| ResourceKernelSpec {
        resource_declaration: resource,
        member: member.into(),
        recipe,
    })
    .collect::<Vec<_>>();
    let checked = match CheckedResourceProgram::prepare(
        parsed,
        HashMap::from([
            (project, SourceOrigin::Project),
            (support, SourceOrigin::Stdlib),
        ]),
        &catalog,
        CheckOptions { release },
    ) {
        Ok(value) => Arc::new(value),
        Err(error) => {
            out["checked_error"] =
                serde_json::json!(format!("{error:?}; {:?}", error.diagnostics()));
            return out;
        }
    };
    out["checked"] = serde_json::json!(true);
    let fields=checked.module().items.iter().filter_map(|item|if let Item::Actor(actor)=item {Some(actor)} else {None}).flat_map(|actor|actor.state_fields.iter()).map(|field|{let callee=if let jett_parser::ast::Expr::Call(callee,_,_)=&field.value {Some(callee.as_ref())} else {None};serde_json::json!({"field":field.name.name,"span":format!("{:?}",field.value.span()),"expression_type":format!("{:?}",checked.checked().type_map.get(&field.value.span())),"ownership":format!("{:?}",checked.checked().call_ownership.get(&field.value.span())),"callee_type":format!("{:?}",callee.and_then(|callee|checked.checked().type_map.get(&callee.span()))),"callee_resolution":format!("{:?}",callee.and_then(|callee|checked.resolved().resolutions.get(&callee.span())))})}).collect::<Vec<_>>();
    out["actor_initializer_facts"] = serde_json::json!(fields);
    let evaluation = jett_driver::evaluate_checked_resource_required_values(&checked);
    out["required_diagnostics"] = serde_json::json!(format!("{:?}", evaluation.diagnostics));
    out["required_event_count"] = serde_json::json!(evaluation.debug_events.len());
    if !evaluation.diagnostics.is_empty() {
        return out;
    }
    out["required"] = serde_json::json!(true);
    let mut interpreter = jett_comptime::Interpreter::new();
    match interpreter.install_checked_resource_program(
        checked.clone(),
        jett_comptime::ExecutionPurpose::ReferenceRuntime,
    ) {
        Ok(()) => {}
        Err(error) => {
            out["reference_install_error"] = serde_json::json!(error);
            return out;
        }
    }
    interpreter.register_module(checked.module());
    interpreter.set_explicit_comptime_values(Arc::new(evaluation.values.clone()));
    out["reference"] = serde_json::json!(format!(
        "{:?}",
        interpreter.call_function("app.main", vec![])
    ));
    out["reference_events"] = serde_json::json!(format!("{:?}", interpreter.take_debug_events()));
    let mut hir = match jett_hir::lower_checked_resource_program(&checked) {
        Ok(value) => value,
        Err(error) => {
            out["hir_error"] = serde_json::json!(format!("{error:?}"));
            return out;
        }
    };
    out["hir"] = serde_json::json!(true);
    if let Err(error) =
        jett_driver::bake_checked_resource_values(&mut hir, &checked, &evaluation.values)
    {
        out["bake_error"] = serde_json::json!(format!("{error:?}"));
        return out;
    }
    out["bake"] = serde_json::json!(true);
    if let Err(error) = jett_hir::complete_value_conversions(&mut hir, &checked.checked().interner)
    {
        out["conversion_error"] = serde_json::json!(format!("{error:?}"));
        return out;
    }
    out["conversion"] = serde_json::json!(true);
    let entry = hir
        .functions
        .iter()
        .filter(|function| {
            function.span == entry
                && function.identity.declaration.origin == SourceOrigin::Project
                && function.identity.declaration.kind == jett_hir::DeclarationKind::Function
        })
        .map(|function| function.id)
        .collect::<Vec<_>>();
    let [entry] = entry.as_slice() else {
        out["entry_error"] = serde_json::json!("not unique");
        return out;
    };
    let mir = match jett_mir::lower(&hir, &checked.checked().interner) {
        Ok(value) => value,
        Err(error) => {
            out["mir_error"] = serde_json::json!(format!("{error:?}"));
            return out;
        }
    };
    out["mir"] = serde_json::json!(true);
    if let Err(error) = jett_mir::validate(&mir) {
        out["mir_validation_error"] = serde_json::json!(format!("{error:?}"));
        return out;
    }
    match jett_codegen_cranelift::emit_host_program_object_with_options(
        &mir,
        &checked.checked().interner,
        *entry,
        jett_codegen_cranelift::CodegenOptions { optimize: release },
    ) {
        Ok(object) => {
            out["object_bytes"] = serde_json::json!(object.bytes.len());
        }
        Err(error) => {
            out["object_error"] = serde_json::json!(format!("{error:?}"));
        }
    }
    out
}
#[test]
fn actor_graph_nested_hook_admission_observation() {
    let rows = vec![observed(SOURCE, false), observed(SOURCE, true)];
    let text = serde_json::to_string_pretty(&rows).unwrap();
    println!("{text}");
    write_observation("nested-hook-admission.json", &text);
    // This captures observations, including failures. Passing this collector
    // does not assert source, reference, object or native acceptance.
}
#[test]
fn actor_graph_nested_hook_capture_admission_observation() {
    let source = include_str!("actor_graph/fixtures/08_nested_hook_capture_actor.jett");
    let rows = vec![observed(source, false), observed(source, true)];
    let text = serde_json::to_string_pretty(&rows).unwrap();
    println!("{text}");
    write_observation("nested-hook-capture-admission.json", &text);
}

fn write_observation(name: &str, text: &str) {
    let directory = std::env::var_os("JETT_ACTOR_GRAPH_EVIDENCE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/actor-graph-tests")
        });
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join(name), text).unwrap();
}

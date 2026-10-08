use super::*;
use jett_common::SourceOrigin;
use jett_comptime::ComptimeContext;
use jett_comptime::checked_types::CheckedExpressionTypes;
use jett_resolve::ResourceKernelSpec;
use jett_typecheck::CheckOptions;
use jett_types::{ReflectionMetadata, ResourceKernelRecipe};
const SUPPORT: &str =
    include_str!("../../jett_driver/tests/native_conformance/resource/resource_probe.jett");
const CLOSE: &str =
    include_str!("../../jett_driver/tests/native_conformance/resource/33_comptime_hook_close.jett");
fn checked(source: &str, release: bool) -> Arc<CheckedResourceProgram> {
    let support = FileId::new(10_000);
    let primary = FileId::new(0);
    let mut parsed = jett_parser::parse(SUPPORT, support);
    let project = jett_parser::parse(source, primary);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert!(project.errors.is_empty(), "{:?}", project.errors);
    let declarations = parsed
        .module
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Resource(value) => Some(value.name.span),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [declaration] = declarations.as_slice() else {
        panic!("one Resource declaration");
    };
    let catalog = [
        ("kernel_create", ResourceKernelRecipe::NetworkFactory),
        ("kernel_borrow", ResourceKernelRecipe::NetworkBorrow),
        ("kernel_close", ResourceKernelRecipe::Finalize),
    ]
    .into_iter()
    .map(|(member, recipe)| ResourceKernelSpec {
        resource_declaration: *declaration,
        member: member.into(),
        recipe,
    })
    .collect::<Vec<_>>();
    parsed.module.items.extend(project.module.items);
    Arc::new(
        CheckedResourceProgram::prepare(
            parsed,
            HashMap::from([
                (support, SourceOrigin::Stdlib),
                (primary, SourceOrigin::Project),
            ]),
            &catalog,
            CheckOptions { release },
        )
        .unwrap_or_else(|error| panic!("{error:?}: {:?}", error.diagnostics())),
    )
}
fn required(checked: &Arc<CheckedResourceProgram>) -> ExplicitComptimeValues {
    let types = Arc::new(CheckedExpressionTypes {
        resource_program: Some(checked.clone()),
        expressions: checked
            .checked()
            .type_map
            .iter()
            .map(|(span, ty)| (*span, checked.checked().interner.type_name(*ty)))
            .collect(),
        ..Default::default()
    });
    let result = jett_comptime::evaluate_explicit_comptime_expressions_capture(
        checked.module(),
        Arc::new(ReflectionMetadata::new()),
        types,
        Arc::new(HashMap::new()),
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(result.debug_events.is_empty());
    result.values
}

#[test]
fn required_source_hooks_and_absence_keep_original_archive_and_derive_exact_view() {
    let sources = [
        CLOSE,
        include_str!(
            "../../jett_driver/tests/native_conformance/resource/34_comptime_hook_factory.jett"
        ),
        include_str!(
            "../../jett_driver/tests/native_conformance/resource/35_comptime_hook_borrow.jett"
        ),
        include_str!(
            "../../jett_driver/tests/native_conformance/resource/40_comptime_hook_absence.jett"
        ),
    ];
    for release in [false, true] {
        for source in sources {
            let checked = checked(source, release);
            let values = required(&checked);
            let mut program = crate::lower_checked_resource_program(&checked).unwrap();
            let original = program.resource_source.functions().to_vec();
            materialize_checked_required_values(&mut program, &values, &checked.checked().interner)
                .unwrap();
            assert!(functions_equal(
                program.resource_source.functions(),
                &original
            ));
            assert!(functions_equal(
                program.resource_source.execution_functions(),
                &program.functions
            ));
            let count = program.resource_source.required_materializations().len();
            assert!(count > 0);
            if source == CLOSE {
                assert!(
                    program
                        .resource_source
                        .required_materializations()
                        .iter()
                        .any(|row| matches!(row.current().kind, E::Int(761)))
                );
            }
            for row in program.resource_source.required_materializations() {
                assert!(matches!(
                    row.original().kind,
                    E::Comptime { .. } | E::Constant { .. }
                ));
                assert_eq!(
                    program.resource_source.materialized_hook(
                        row.function(),
                        row.original(),
                        row.current()
                    ),
                    row.hook()
                );
                if let Some(hook) = row.hook() {
                    assert!(
                        matches!(&row.current().kind, E::ResourceHookValue { hook: current } if current == hook)
                    );
                    assert_eq!(row.current().ty, hook.function_type());
                    assert_eq!(row.current().span, row.original().span);
                }
            }
            materialize_checked_required_values(&mut program, &values, &checked.checked().interner)
                .unwrap();
            assert_eq!(
                program.resource_source.required_materializations().len(),
                count
            );
        }
    }
}

#[test]
fn foreign_missing_and_public_mirror_values_never_materialize_resource_authority() {
    for release in [false, true] {
        let checked = checked(CLOSE, release);
        let values = required(&checked);
        let foreign = self::checked(CLOSE, release);
        let mut program = crate::lower_checked_resource_program(&foreign).unwrap();
        assert!(program.resource_source.belongs_to(&foreign));
        assert!(!program.resource_source.belongs_to(&checked));
        let before = program.clone();
        assert!(
            materialize_checked_required_values(&mut program, &values, &foreign.checked().interner)
                .is_err()
        );
        assert_eq!(program, before);
        let mut program = crate::lower_checked_resource_program(&checked).unwrap();
        let proof = values
            .checked_required_values(&checked)
            .unwrap()
            .into_iter()
            .find(|v| v.checked_hook(&checked).unwrap().is_some())
            .unwrap();
        let mut mirror = ExplicitComptimeValues::default();
        mirror.insert(
            proof.source_span(),
            ComptimeContext::default(),
            proof.value().clone(),
        );
        assert!(
            materialize_checked_required_values(&mut program, &mirror, &checked.checked().interner)
                .is_err()
        );
        assert!(
            materialize_checked_required_values(
                &mut program,
                &ExplicitComptimeValues::default(),
                &checked.checked().interner
            )
            .is_err()
        );
        let mut overwritten = values.clone();
        overwritten.insert(
            proof.source_span(),
            ComptimeContext::default(),
            Value::Int64(99),
        );
        materialize_checked_required_values(
            &mut program,
            &overwritten,
            &checked.checked().interner,
        )
        .unwrap();
        assert!(
            program
                .resource_source
                .required_materializations()
                .iter()
                .any(|row| row.hook().is_some())
        );
    }
}

#[test]
fn altered_current_bodies_headers_and_materialized_hooks_refuse_atomically() {
    for release in [false, true] {
        let checked = checked(CLOSE, release);
        let values = required(&checked);
        let original = crate::lower_checked_resource_program(&checked).unwrap();
        let main = original
            .functions
            .iter()
            .position(|f| f.identity.declaration.name == "main")
            .unwrap();
        let mut changed = original.clone();
        changed.functions[main]
            .identity
            .declaration
            .name
            .push_str("_forged");
        let before = changed.clone();
        assert!(
            materialize_checked_required_values(&mut changed, &values, &checked.checked().interner)
                .is_err()
        );
        assert_eq!(changed, before);
        let mut changed = original.clone();
        changed.functions[main].body.statements.pop();
        assert!(
            materialize_checked_required_values(&mut changed, &values, &checked.checked().interner)
                .is_err()
        );
        let mut changed = original.clone();
        let mut replaced = false;
        walk_block(&mut changed.functions[main].body, &mut |expression| {
            if matches!(expression.kind, E::Comptime { .. }) && !replaced {
                expression.kind = E::ResourceHookValue {
                    hook: original
                        .resource_manifest
                        .hooks()
                        .find(|h| h.recipe() == ResourceKernelRecipe::Finalize)
                        .unwrap(),
                };
                replaced = true;
            }
            Ok(())
        })
        .unwrap();
        assert!(replaced);
        assert!(
            materialize_checked_required_values(&mut changed, &values, &checked.checked().interner)
                .is_err()
        );
        let mut baked = original.clone();
        materialize_checked_required_values(&mut baked, &values, &checked.checked().interner)
            .unwrap();
        walk_block(&mut baked.functions[main].body, &mut |expression| {
            if matches!(expression.kind, E::ResourceHookValue { .. }) {
                expression.ty = TypeInterner::BOOL;
            }
            Ok(())
        })
        .unwrap();
        assert!(
            materialize_checked_required_values(&mut baked, &values, &checked.checked().interner)
                .is_err()
        );
    }
}

#[test]
fn required_only_generic_and_scoped_helpers_keep_original_body_and_exports() {
    let sources = [
        include_str!(
            "../../jett_driver/tests/native_conformance/resource/41_comptime_hook_pure_choice.jett"
        ),
        include_str!(
            "../../jett_driver/tests/native_conformance/resource/42_comptime_hook_generic.jett"
        ),
        include_str!(
            "../../jett_driver/tests/native_conformance/resource/43_comptime_hook_scoped.jett"
        ),
    ];
    for release in [false, true] {
        for source in sources {
            let checked = checked(source, release);
            let values = required(&checked);
            let mut program = crate::lower_checked_resource_program(&checked).unwrap();
            materialize_checked_required_values(&mut program, &values, &checked.checked().interner)
                .unwrap();
            let choose = program
                .functions
                .iter()
                .find(|f| f.identity.declaration.name == "choose")
                .unwrap();
            assert!(
                program
                    .resource_source
                    .required_only_function_ids()
                    .contains(&choose.id)
            );
            assert!(
                program
                    .resource_source
                    .functions()
                    .iter()
                    .any(|f| f.id == choose.id && f.body == choose.body)
            );
            assert!(
                program
                    .resource_source
                    .required_only_function_ids()
                    .iter()
                    .all(|id| !program.resource_source.exported_function_ids().contains(id))
            );
            let exported = source.replacen("function choose", "export function choose", 1);
            let checked = self::checked(&exported, release);
            let values = required(&checked);
            let mut program = crate::lower_checked_resource_program(&checked).unwrap();
            materialize_checked_required_values(&mut program, &values, &checked.checked().interner)
                .unwrap();
            let choose = program
                .functions
                .iter()
                .find(|f| f.identity.declaration.name == "choose")
                .unwrap();
            assert!(
                program
                    .resource_source
                    .exported_function_ids()
                    .contains(&choose.id)
            );
            assert!(
                !program
                    .resource_source
                    .required_only_function_ids()
                    .contains(&choose.id)
            );
        }
    }
}

#[test]
fn materialized_float_authentication_retains_nan_payload_and_signed_zero_bits() {
    let span = Span::new(FileId::new(0), 1, 2);
    let a = Expression {
        kind: E::Float(f64::from_bits(0x7ff8_0000_0000_0001)),
        ty: TypeInterner::FLOAT64,
        span,
    };
    let mut b = a.clone();
    assert!(expression_equal(&a, &b));
    b.kind = E::Float(f64::from_bits(0x7ff8_0000_0000_0002));
    assert!(!expression_equal(&a, &b));
    b.kind = E::Float(0.0);
    let mut c = b.clone();
    c.kind = E::Float(-0.0);
    assert!(!expression_equal(&b, &c));
}

#[test]
fn actual_generic_and_nested_scope_required_nodes_join_full_context() {
    let source = include_str!(
        "../../jett_comptime/src/resource_execution/fixtures/27_original_generic_scoped_required.jett"
    );
    for release in [false, true] {
        let checked = checked(source, release);
        let values = required(&checked);
        let mut program = crate::lower_checked_resource_program(&checked).unwrap();
        materialize_checked_required_values(&mut program, &values, &checked.checked().interner)
            .unwrap();
        let rows = program.resource_source.required_materializations();
        let generic = rows
            .iter()
            .find(|row| row.proof.generic_index().is_some())
            .expect("actual generic body required value");
        let scoped = rows
            .iter()
            .find(|row| row.proof.scoped_bindings().len() == 2)
            .expect("actual two-scope required value");
        for row in [generic, scoped] {
            assert!(matches!(&row.current().kind, E::String(value) if value == "int64"));
            let function = program
                .resource_source
                .functions()
                .iter()
                .find(|function| function.id == row.function())
                .unwrap();
            assert!(proof_matches(
                &row.proof,
                function,
                row.original(),
                &checked
            ));
            let mut copied = row.original().clone();
            if let E::Comptime { source_span, .. } = &mut copied.kind {
                source_span.start += 1;
            }
            assert!(!proof_matches(&row.proof, function, &copied, &checked));
        }
        let function = program
            .resource_source
            .functions()
            .iter()
            .find(|function| function.id == generic.function())
            .unwrap();
        let mut changed = function.clone();
        changed
            .identity
            .specialization
            .type_info_kinds
            .push((0, "forged".into()));
        assert!(!proof_matches(
            &generic.proof,
            &changed,
            generic.original(),
            &checked
        ));
        let function = program
            .resource_source
            .functions()
            .iter()
            .find(|function| function.id == scoped.function())
            .unwrap();
        for field in [0, 1, 2] {
            let mut changed = scoped.original().clone();
            let E::Comptime { scopes, .. } = &mut changed.kind else {
                panic!("retained required node");
            };
            match field {
                0 => scopes[0].index += 1,
                1 => scopes[0].owner.start += 1,
                _ => {
                    scopes[0].selection =
                        jett_typecheck::CheckedComptimeTypeSelection::ReflectedIteration(99)
                }
            }
            assert!(!proof_matches(&scoped.proof, function, &changed, &checked));
        }
    }
}

#[test]
fn mutual_required_body_uses_retained_executable_declaration() {
    let source = "namespace app\nmutual:\n    function first(flag: bool) returns int64\n    function second() returns int64\nfunction first(flag: bool) returns int64:\n    int64 literal = comptime 7\n    if flag:\n        return second()\n    return literal\nfunction second() returns int64:\n    return first(false)\nexport function main(net: Network) returns nothing:\n    int64 selected = first(true)\n    return nothing\n";
    for release in [false, true] {
        let checked = checked(source, release);
        let values = required(&checked);
        let proofs = values.checked_required_values(&checked).unwrap();
        let [proof] = proofs.as_slice() else {
            panic!("one closed required literal in the mutual body");
        };
        let CheckedRequiredOwner::Function {
            definition,
            declaration,
        } = proof.owner()
        else {
            panic!("actual required function owner");
        };
        assert_ne!(
            checked.resolved().scope_table.def(definition).span,
            declaration,
            "resolver keeps the earlier mutual signature; proof retains the executable body"
        );
        let mut program = crate::lower_checked_resource_program(&checked).unwrap();
        materialize_checked_required_values(&mut program, &values, &checked.checked().interner)
            .unwrap();
        let [row] = program.resource_source.required_materializations() else {
            panic!("one materialized required literal");
        };
        assert!(matches!(row.current().kind, E::Int(7)));
        assert_eq!(
            program
                .functions
                .iter()
                .find(|function| function.id == row.function())
                .unwrap()
                .source_definition,
            Some(definition)
        );
    }
}

//! Required inline and refinement values retain their exact original owner.
use super::*;

const INLINE_SOURCE: &str = "namespace app\nfunction callback[T]() returns function() returns string:\n    return function() returns string: return comptime type.name[T]()\nfunction scoped[T]() returns function() returns string:\n    comptime type Bound = type.info[T]():\n        return function() returns string: return comptime type.name[Bound]()\nfunction nested() returns function() returns function() returns int64:\n    return function() returns function() returns int64:\n        return function() returns int64: return comptime 7\nfunction main() returns nothing:\n    function() returns string small = callback[int8]()\n    function() returns string text = callback[string]()\n    function() returns string bound = scoped[int16]()\n    function() returns function() returns int64 factory = nested()\n    function() returns int64 inner = factory()\n    return nothing\n";

#[test]
fn required_inline_parentheses_preserve_the_physical_source_association() {
    let source = "namespace app\nfunction callback() returns function() returns int64:\n    return ((function() returns int64: return comptime 7))\nfunction main() returns nothing:\n    function() returns int64 selected = callback()\n    return nothing\n";
    for release in [false, true] {
        let checked = checked(source, release);
        let values = required(&checked);
        let mut program = crate::lower_checked_resource_program(&checked).unwrap();
        materialize_checked_required_values(&mut program, &values, &checked.checked().interner)
            .unwrap();
        let [row] = program.resource_source.required_materializations() else {
            panic!("one required parenthesized inline value")
        };
        assert!(matches!(row.current().kind, E::Int(7)));
        let function = program
            .resource_source
            .functions()
            .iter()
            .find(|f| f.id == row.function())
            .unwrap();
        let inline = row.proof.original_inline_function().unwrap().unwrap();
        assert_ne!(
            function.span,
            inline.span(),
            "the extracted span retains both parentheses"
        );
        assert!(
            program
                .resource_source
                .original_inline_root(function, row.original(), inline, &checked)
                .is_some()
        );
    }
}

#[test]
fn parenthesized_required_inline_expansions_keep_each_scoped_selection() {
    let source = "namespace app\nstruct Pair:\n    first: int8\n    second: int16\nfunction labels() returns nothing:\n    for field in type.fields[Pair]():\n        comptime type Field = field.type_info:\n            function() returns string label = (function() returns string: return comptime type.name[Field]())\n    return nothing\nfunction main() returns nothing:\n    labels()\n    return nothing\n";
    for release in [false, true] {
        let checked = checked(source, release);
        let values = required(&checked);
        let mut program = crate::lower_checked_resource_program(&checked).unwrap();
        materialize_checked_required_values(&mut program, &values, &checked.checked().interner)
            .unwrap();
        let rows = program.resource_source.required_materializations();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].proof.source_span(), rows[1].proof.source_span());
        assert_ne!(
            rows[0].proof.scoped_bindings(),
            rows[1].proof.scoped_bindings()
        );
        for row in rows {
            let function = program
                .resource_source
                .functions()
                .iter()
                .find(|f| f.id == row.function())
                .unwrap();
            assert!(proof_matches(
                &row.proof,
                function,
                row.original(),
                &checked,
                &program.resource_source
            ));
            let sibling = rows
                .iter()
                .find(|other| other.function() != row.function())
                .unwrap();
            assert!(!proof_matches(
                &sibling.proof,
                function,
                row.original(),
                &checked,
                &program.resource_source
            ));
        }
    }
}

#[test]
fn required_inline_values_match_exact_extraction_root_and_complete_context() {
    for release in [false, true] {
        let checked = checked(INLINE_SOURCE, release);
        let values = required(&checked);
        let mut program = crate::lower_checked_resource_program(&checked).unwrap();
        let original = program.resource_source.functions().to_vec();
        materialize_checked_required_values(&mut program, &values, &checked.checked().interner)
            .unwrap();
        assert!(functions_equal(
            program.resource_source.functions(),
            &original
        ));
        let rows = program.resource_source.required_materializations();
        assert_eq!(rows.len(), 4);
        assert_eq!(
            rows.iter()
                .filter(|row| row.proof.generic_index().is_some())
                .count(),
            3
        );
        assert!(
            rows.iter()
                .any(|row| row.proof.scoped_bindings().len() == 1)
        );
        assert!(
            rows.iter()
                .any(|row| matches!(&row.current().kind, E::Int(7)))
        );
        for row in rows {
            let function = original.iter().find(|f| f.id == row.function()).unwrap();
            assert_eq!(function.debug_kind, crate::FunctionDebugKind::Inline);
            assert!(function.source_definition.is_none());
            assert!(proof_matches(
                &row.proof,
                function,
                row.original(),
                &checked,
                &program.resource_source
            ));
            let inline = row.proof.original_inline_function().unwrap().unwrap();
            assert!(
                program
                    .resource_source
                    .original_inline_root(function, row.original(), inline, &checked)
                    .is_some()
            );
            let copied = inline.clone();
            assert!(
                program
                    .resource_source
                    .original_inline_root(function, row.original(), &copied, &checked)
                    .is_none()
            );
            assert!(!proof_matches(
                &row.proof,
                function,
                row.original(),
                &checked,
                &crate::ResourceSourceArchive::empty()
            ));
            for field in 0..5 {
                let mut changed = function.clone();
                match field {
                    0 => changed.identity.declaration.namespace = "forged".into(),
                    1 => changed.return_type = TypeInterner::BOOL,
                    2 => changed
                        .identity
                        .specialization
                        .type_info_kinds
                        .push((0, "forged".into())),
                    3 => changed.span.start += 1,
                    _ => changed.identity.scoped_type_bindings.clear(),
                }
                if changed != *function {
                    assert!(!proof_matches(
                        &row.proof,
                        &changed,
                        row.original(),
                        &checked,
                        &program.resource_source
                    ));
                }
            }
            let mut expression = row.original().clone();
            let E::Comptime { value, .. } = &mut expression.kind else {
                panic!("required inline node")
            };
            value.kind = E::Bool(false);
            assert!(!proof_matches(
                &row.proof,
                function,
                &expression,
                &checked,
                &program.resource_source
            ));
            let sibling = original
                .iter()
                .find(|f| f.id != function.id && f.debug_kind == crate::FunctionDebugKind::Inline)
                .unwrap();
            let mut copied_owner = sibling.clone();
            copied_owner.span = function.span;
            assert!(!proof_matches(
                &row.proof,
                &copied_owner,
                row.original(),
                &checked,
                &program.resource_source
            ));
        }
        let foreign = self::checked(INLINE_SOURCE, release);
        let foreign_values = required(&foreign);
        let foreign_proofs = foreign_values.checked_required_values(&foreign).unwrap();
        for row in rows {
            let function = original.iter().find(|f| f.id == row.function()).unwrap();
            let foreign_proof = foreign_proofs
                .iter()
                .find(|proof| {
                    proof.source_span() == row.proof.source_span()
                        && proof.type_arguments() == row.proof.type_arguments()
                })
                .unwrap();
            assert!(!proof_matches(
                foreign_proof,
                function,
                row.original(),
                &checked,
                &program.resource_source
            ));
        }
    }
}

const REFINEMENT_SOURCE: &str = "namespace domain\ntype Positive = int64 where value > 0 and (comptime 3) > 0\ntype Other = int64 where value > 0 and (comptime 4) > 0\nnamespace app\nfunction main() returns nothing:\n    return nothing\n";

#[test]
fn required_refinement_values_match_original_predicate_and_checked_declaration() {
    for release in [false, true] {
        let checked = checked(REFINEMENT_SOURCE, release);
        let values = required(&checked);
        let mut program = crate::lower_checked_resource_program(&checked).unwrap();
        materialize_checked_required_values(&mut program, &values, &checked.checked().interner)
            .unwrap();
        let rows = program.resource_source.required_materializations();
        assert_eq!(rows.len(), 2);
        for row in rows {
            let function = program
                .resource_source
                .functions()
                .iter()
                .find(|f| f.id == row.function())
                .unwrap();
            assert_eq!(
                function.identity.declaration.kind,
                DeclarationKind::RefinementPredicate
            );
            assert!(matches!(
                row.proof.owner(),
                CheckedRequiredOwner::RefinementPredicate { .. }
            ));
            assert!(row.proof.original_inline_function().unwrap().is_none());
            assert!(proof_matches(
                &row.proof,
                function,
                row.original(),
                &checked,
                &program.resource_source
            ));
            for field in 0..6 {
                let mut changed = function.clone();
                match field {
                    0 => changed.source_definition = None,
                    1 => changed.identity.declaration.namespace = "app".into(),
                    2 => changed.identity.declaration.kind = DeclarationKind::Function,
                    3 => changed.params.clear(),
                    4 => {
                        changed.identity.declaration.name =
                            if function.identity.declaration.name == "Positive" {
                                "Other".into()
                            } else {
                                "Positive".into()
                            }
                    }
                    _ => {
                        changed.identity.declaration.name = format!(
                            "{}.{}",
                            function.identity.declaration.namespace,
                            function.identity.declaration.name,
                        )
                    }
                }
                assert!(!proof_matches(
                    &row.proof,
                    &changed,
                    row.original(),
                    &checked,
                    &program.resource_source
                ));
            }
            let sibling = rows
                .iter()
                .find(|other| other.function() != row.function())
                .unwrap();
            assert!(!proof_matches(
                &sibling.proof,
                function,
                row.original(),
                &checked,
                &program.resource_source
            ));
            let mut changed = row.original().clone();
            let E::Comptime { value, .. } = &mut changed.kind else {
                panic!("required predicate node")
            };
            value.kind = E::Int(9);
            assert!(!proof_matches(
                &row.proof,
                function,
                &changed,
                &checked,
                &program.resource_source
            ));
        }
        let foreign = self::checked(REFINEMENT_SOURCE, release);
        let foreign_values = required(&foreign);
        for foreign_proof in foreign_values.checked_required_values(&foreign).unwrap() {
            let row = rows
                .iter()
                .find(|row| row.proof.source_span() == foreign_proof.source_span())
                .unwrap();
            let function = program
                .resource_source
                .functions()
                .iter()
                .find(|f| f.id == row.function())
                .unwrap();
            assert!(!proof_matches(
                &foreign_proof,
                function,
                row.original(),
                &checked,
                &program.resource_source
            ));
        }
    }
}

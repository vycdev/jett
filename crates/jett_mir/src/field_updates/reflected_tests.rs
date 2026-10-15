//! Source-derived ordinal composition and wrong-arm proof controls.
use super::*;
use jett_common::{FileId, SourceOrigin};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::Arc,
};

fn source_program() -> (
    hir::Program,
    Program,
    Arc<jett_typecheck::CheckedResourceProgram>,
) {
    // Shared genuine Driver Source; keep its existing report body unchanged.
    let source = include_str!(
        "../../../../tests/native/field_update_reference/31_reflected_iteration_update.jett"
    );
    let source = source
        .split("function main(")
        .next()
        .expect("report precedes main");
    let file = FileId::new(0);
    let list_file = FileId::new(jett_common::STDLIB_FILE_ID_START);
    let mut parsed = jett_parser::parse(include_str!("../../../../stdlib/list.jett"), list_file);
    let mut project = jett_parser::parse(source, file);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert!(project.errors.is_empty(), "{:?}", project.errors);
    // Preserve the genuine report AST and its spans; supply its shipped list
    // API through the same separate-file stdlib merge used by other MIR tests.
    parsed.module.items.append(&mut project.module.items);
    let original = Arc::new(
        jett_typecheck::CheckedResourceProgram::prepare(
            parsed,
            HashMap::from([
                (list_file, SourceOrigin::Stdlib),
                (file, SourceOrigin::Project),
            ]),
            &[],
            jett_typecheck::CheckOptions::default(),
        )
        .expect("normally checked ordinary reflected Source"),
    );
    let high = hir::lower_checked_ordinary_program(&original).expect("original ordinal HIR");
    let program = crate::lower(&high, &original.checked().interner).expect("same-span ordinal MIR");
    (high, program, original)
}

#[test]
fn same_outer_field_writes_keep_all_three_original_ordinal_contexts() {
    let (high, mut program, original) = source_program();
    let high_report = high
        .functions
        .iter()
        .find(|f| f.identity.declaration.name == "report")
        .unwrap();
    let places = hir::field_update_places(&high_report.body);
    assert_eq!(places.len(), 9);
    assert_eq!(
        places
            .iter()
            .map(|place| place.root())
            .collect::<HashSet<_>>()
            .len(),
        1
    );
    assert_eq!(
        places
            .iter()
            .map(|place| place.occurrence())
            .collect::<BTreeSet<_>>()
            .len(),
        9
    );
    let ordinals = places
        .iter()
        .flat_map(|place| place.required_scopes())
        .filter_map(|scope| {
            if let hir::FieldUpdateScopeSelection::ReflectedIteration(ordinal) = scope.selection {
                Some(ordinal)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    for ordinal in 0..3 {
        assert_eq!(
            ordinals
                .iter()
                .filter(|selected| **selected == ordinal)
                .count(),
            3
        );
    }
    validate_field_updates(&program, &original.checked().interner).unwrap();
    crate::prepare_native_sequences(&mut program, &original.checked().interner);
    crate::prepare_native_uninhabited_sums(&mut program, &original.checked().interner);
    crate::prepare_native_generated_functions(&mut program, &original.checked().interner);
    validate_field_updates(&program, &original.checked().interner)
        .expect("exact ordinal Source proof survives canonical preparation");
}

#[test]
fn fresh_seal_refuses_same_shape_place_copied_or_swapped_between_ordinal_arms() {
    let (high, program, original) = source_program();
    let high_report = high
        .functions
        .iter()
        .find(|f| f.identity.declaration.name == "report")
        .unwrap();
    let report = program
        .functions
        .iter()
        .find(|f| f.identity.declaration.name == "report")
        .unwrap();
    let mut sites = Vec::new();
    for (block, body) in report.blocks.iter().enumerate() {
        for (index, statement) in body.statements.iter().enumerate() {
            if let StatementKind::UpdateField { place, .. } = &statement.kind {
                if place.span() == hir::field_update_places(&high_report.body)[0].span() {
                    sites.push((block, index, place.clone()));
                }
            }
        }
    }
    assert_eq!(sites.len(), 3);
    for swap in [false, true] {
        let mut changed = report.clone();
        // Ordinals 0 and 2 both bind int64 and update the same outer root/path.
        // Swap every matching begin/commit pair; unique paired keys and the
        // complete original Source-place multiset remain intact in swap=true.
        let first = &sites[0].2;
        let second = &sites[2].2;
        assert_eq!(first.root(), second.root());
        assert_eq!(first.target(), second.target());
        assert_eq!(first.scoped_type_bindings(), second.scoped_type_bindings());
        for body in &mut changed.blocks {
            for statement in &mut body.statements {
                if let StatementKind::PrepareFieldUpdate { place, .. }
                | StatementKind::UpdateField { place, .. } = &mut statement.kind
                {
                    if place.occurrence() == second.occurrence() {
                        *place = first.clone();
                    } else if swap && place.occurrence() == first.occurrence() {
                        *place = second.clone();
                    }
                }
            }
        }
        let error = seal(high_report, &changed, &original.checked().interner)
            .expect_err("wrong original ordinal placement must not mint a field witness");
        if swap {
            assert!(
                error.contains("initially authenticated Source"),
                "independent original/current archive gate: {error}"
            );
        }
    }
}

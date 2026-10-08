use super::*;
#[path = "../../../../jett_driver/tests/native_conformance/resource_cases.rs"]
mod cases;

#[test]
fn resource_borrowed_sum_layout_retains_exact_incoming_shape_and_projection_issuer() {
    let cases = cases::CASES
        .iter()
        .filter(|case| {
            case.name.starts_with("view_optional_") || case.name.starts_with("view_result_")
        })
        .collect::<Vec<_>>();
    assert_eq!(cases.len(), 12);
    for release in [false, true] {
        for case in &cases {
            let checked = checked(case.source, release);
            let program = lower_source(&checked);
            let layout = EmittedResourceLayout::from_program(
                &program,
                &checked.checked().interner,
                entry(&program),
            )
            .unwrap_or_else(|error| panic!("{} release={release}: {error}", case.name));
            let mut rows = Rows::new(layout.plan()).unwrap();
            rows.populate().unwrap();
            let helper = layout
                .plan()
                .functions()
                .iter()
                .find(|function| {
                    function.identity().declaration.namespace == "app"
                        && function.identity().declaration.name.starts_with("observe_")
                })
                .unwrap();
            let (parameter, header) = helper
                .parameters()
                .iter()
                .enumerate()
                .find(|(_, header)| header.name == "outcome")
                .unwrap();
            assert_eq!(header.mode, jett_mir::ParamMode::View);
            assert!(helper.owner_slots().iter().all(|slot|
                !matches!(slot.storage(),custody::ResourceSlotStorage::Local { header: stored } if stored.id == header.local)));
            let incoming = helper.loans().iter().find(|loan|
                matches!(loan.source(),custody::ResourceLoanSource::IncomingViewFormal { parameter: original, .. } if original == parameter)).unwrap();
            assert!(!matches!(
                incoming.shape(),
                custody::ResourceShape::Plain { .. }
            ));
            let projections = helper
                .operations()
                .iter()
                .filter(|operation| matches!(operation.role(), Role::ProjectSumView { .. }))
                .collect::<Vec<_>>();
            assert_eq!(projections.len(), 1, "{} release={release}", case.name);
            let projection = projections[0];
            let Role::ProjectSumView {
                source,
                destination,
                path,
                ..
            } = projection.role()
            else {
                unreachable!()
            };
            assert_eq!(*source, incoming.id());
            assert!(matches!(helper.loans()[destination.index()].source(),
                custody::ResourceLoanSource::ProjectedSumPayload { parent, path: selected, .. }
                    if parent == *source && selected == *path));
            let row_id = layout
                .operation(helper.function(), projection.id())
                .unwrap();
            assert_eq!(rows.loan(helper, *destination).unwrap(), [3, row_id]);
            let row = &rows.operations[row_id as usize].1;
            assert_eq!(row[0], 23);
            assert_eq!(row[2], 2);
            assert_eq!(row[4], parameter as u32);
            assert!(helper.operations().iter().any(|operation|
                matches!(operation.role(),Role::ObserveSumView { source: observed, .. } if observed == source)));
            assert!(helper.operations().iter().all(|operation|
                !matches!(operation.role(),Role::EndSumBorrow { loan } if *loan == incoming.id())));
        }
    }
}

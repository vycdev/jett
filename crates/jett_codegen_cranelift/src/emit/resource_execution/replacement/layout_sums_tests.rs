//! Genuine direct-sum sources retain their exact typed replacement rows.
use super::*;

const SOURCES: &[&str] = &[
    include_str!("optional_transitions.jett"),
    include_str!("result_transitions.jett"),
    include_str!("primitive_failure.jett"),
    include_str!("conditional_optional.jett"),
    include_str!("conditional_result.jett"),
];

#[test]
fn resource_layout_direct_sum_replacements_preserve_shape_occupancy_and_scope() {
    for release in [false, true] {
        let mut saw_empty = false;
        let mut saw_occupied = false;
        let mut saw_conditional = false;
        let mut saw_optional = false;
        let mut saw_result = false;
        for source in SOURCES {
            let checked = checked(source, release);
            let program = lower_source(&checked);
            let layout = EmittedResourceLayout::from_program(
                &program,
                &checked.checked().interner,
                entry(&program),
            )
            .unwrap();
            let mut rows = Rows::new(layout.plan()).unwrap();
            rows.populate().unwrap();
            let mut replacements = 0;
            for function in layout.plan().functions() {
                for operation in function.operations() {
                    let Role::Replace {
                        destination,
                        replacement,
                        old,
                    } = operation.role()
                    else {
                        continue;
                    };
                    replacements += 1;
                    let destination_slot = &function.owner_slots()[destination.index()];
                    let rhs_slot = &function.owner_slots()[replacement.index()];
                    assert_ne!(destination, replacement);
                    assert_eq!(destination_slot.shape(), rhs_slot.shape());
                    assert_eq!(destination_slot.frame(), operation.frame());
                    assert_eq!(rhs_slot.frame(), operation.frame());
                    assert_eq!(
                        function.frames()[operation.frame().index()].role(),
                        custody::ResourceFrameRole::Scope
                    );
                    assert!(matches!(destination_slot.storage(),
                        custody::ResourceSlotStorage::Local { header } if header.mutable));
                    match old {
                        custody::ResourceOccupancy::Empty => saw_empty = true,
                        custody::ResourceOccupancy::Occupied => saw_occupied = true,
                        custody::ResourceOccupancy::Conditional => saw_conditional = true,
                    }
                    match destination_slot.shape() {
                        custody::ResourceShape::Optional { .. } => saw_optional = true,
                        custody::ResourceShape::Result { .. } => saw_result = true,
                        custody::ResourceShape::Plain { .. } => {
                            panic!("sum Source unexpectedly uses Plain replacement")
                        }
                    }
                    let ordinal = layout
                        .operation(function.function(), operation.id())
                        .unwrap();
                    assert_eq!(
                        rows.operations[ordinal as usize].1,
                        [
                            12,
                            layout
                                .frame(function.function(), operation.frame())
                                .unwrap(),
                            layout.slot(function.function(), *destination).unwrap(),
                            layout.slot(function.function(), *replacement).unwrap(),
                        ]
                    );
                }
            }
            assert!(replacements >= 2);
        }
        assert!(saw_empty && saw_occupied && saw_conditional && saw_optional && saw_result);
    }
}

use super::*;

fn wrapper(values: &mut NativeValues, kind: u32) -> u64 {
    match kind {
        2 => values.new_list(false).unwrap(),
        5 => values.new_set(0).unwrap(),
        6 => values.new_map(0, 0).unwrap(),
        7 => values.sum(SUM_SUCCESS, 7, false).unwrap(),
        _ => unreachable!(),
    }
}

fn pending(values: &mut NativeValues, source: u64, kind: u32) -> u64 {
    match kind {
        2 => values.run_list(source).unwrap(),
        5 => values.run_set(source).unwrap(),
        6 => values.run_map(source).unwrap(),
        7 => values.run_sum(source).unwrap(),
        _ => unreachable!(),
    }
}

#[test]
fn reflected_container_readiness_preserves_source_and_exact_depth_without_observation() {
    for (kind, message) in [
        (2, "reflected field refinement: expected a ready list value"),
        (5, "reflected field refinement: expected a ready set value"),
        (6, "reflected field refinement: expected a ready map value"),
        (
            7,
            "reflected field refinement: expected a ready optional value",
        ),
        (
            7,
            "reflected field refinement: expected a ready result value",
        ),
    ] {
        let mut values = NativeValues::default();
        let source = wrapper(&mut values, kind);
        let once = pending(&mut values, source, kind);
        let twice = pending(&mut values, once, kind);
        assert_eq!(
            values.reject_pending_handle(source, kind, message.as_bytes()),
            Ok(0)
        );
        assert!(values.dynamic_failure_message.is_none());
        for (value, depth) in [(once, 1), (twice, 2)] {
            assert!(
                values
                    .reject_pending_handle(value, kind, message.as_bytes())
                    .is_err()
            );
            assert_eq!(
                values.dynamic_failure_message.as_deref(),
                Some(message.as_bytes())
            );
            assert_eq!(values.owned_pending_depth(value), Ok(depth));
            assert_eq!(values.owned_pending_depth(source), Ok(0));
        }
        for value in [source, once, twice] {
            values.drop_value(value).unwrap();
        }
        assert!(values.is_empty());
    }
}

#[test]
fn reflected_container_readiness_rejects_wrong_kind_foreign_stale_and_unknown_tags() {
    let mut values = NativeValues::default();
    let mut foreign = NativeValues::default();
    let message = b"reflected field refinement: expected a ready map value";
    for kind in [2, 5, 6, 7] {
        let source = wrapper(&mut values, kind);
        for wrong in [2, 5, 6, 7].into_iter().filter(|wrong| *wrong != kind) {
            assert!(
                values
                    .reject_pending_handle(source, wrong, message)
                    .is_err()
            );
            assert!(values.dynamic_failure_message.is_none());
            assert_eq!(values.owned_pending_depth(source), Ok(0));
        }
        let alien = wrapper(&mut foreign, kind);
        assert!(values.reject_pending_handle(alien, kind, message).is_err());
        assert!(values.dynamic_failure_message.is_none());
        foreign.drop_value(alien).unwrap();
        assert!(
            values
                .reject_pending_handle(source, u32::MAX, message)
                .is_err()
        );
        values.drop_value(source).unwrap();
        assert!(values.reject_pending_handle(source, kind, message).is_err());
        assert!(values.dynamic_failure_message.is_none());
    }
    assert!(values.is_empty());
    assert!(foreign.is_empty());
}

#[test]
fn reflected_container_readiness_ignores_pending_children_and_keeps_owned_payloads() {
    let mut values = NativeValues::default();
    let list = values.new_list(false).unwrap();
    values.lists.get_mut(&list).unwrap().elements.push(Some(7));
    values
        .lists
        .get_mut(&list)
        .unwrap()
        .element_pending_depths
        .insert(0, 2);
    let text = values.insert("Agent-λ🙂".into()).unwrap();
    values.strings.get_mut(&text).unwrap().pending_depth = 2;
    let sum = values.sum(SUM_SUCCESS, text, true).unwrap();
    for (value, kind) in [(list, 2), (sum, 7)] {
        assert_eq!(
            values.reject_pending_handle(value, kind, b"must not inspect a child"),
            Ok(0)
        );
    }
    assert!(values.dynamic_failure_message.is_none());
    assert_eq!(values.lists[&list].element_pending_depths[&0], 2);
    assert_eq!(values.strings[&text].pending_depth, 2);
    assert_eq!(values.strings[&text].text, "Agent-λ🙂");
    values.drop_value(list).unwrap();
    values.drop_value(sum).unwrap();
    assert!(values.is_empty());
}

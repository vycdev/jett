//! Private forged-proof negatives; these mint no public Source or ABI authority.
use super::*;

#[test]
fn ordinary_attempt_owner_checks_every_retained_identity_component() {
    context(|auth| {
        install(auth, &[], 1);
        let (attempt, _, _, _) = start(auth, ResourcePurpose::Runtime);
        auth.with_state(|_, state| {
            let owner = state
                .resource_state
                .as_ref()
                .unwrap()
                .attempt
                .as_ref()
                .unwrap()
                .ordinary_owner;
            state
                .values
                .validate_resource_ordinary_attempt(owner)
                .unwrap();
            let mut wrong_context = owner;
            wrong_context.context.token = wrong_context.context.token.wrapping_add(1);
            let mut wrong_program = owner;
            wrong_program.program = ResourceHandleId::new(owner.program.raw() + 1).unwrap();
            let mut wrong_entry = owner;
            wrong_entry.entry.function += 1;
            let mut wrong_signature = owner;
            wrong_signature.entry.signature += 1;
            let mut wrong_scope = owner;
            wrong_scope.entry.scope += 1;
            let mut wrong_root = owner;
            wrong_root.root = ResourceHandleId::new(owner.root.raw() + 1).unwrap();
            let mut wrong_attempt = owner;
            wrong_attempt.attempt = ResourceHandleId::new(owner.attempt.raw() + 1).unwrap();
            let mut wrong_purpose = owner;
            wrong_purpose.purpose = ResourcePurpose::Property;
            let mut wrong_ordinal = owner;
            wrong_ordinal.ordinal += 1;
            for forged in [
                wrong_context,
                wrong_program,
                wrong_entry,
                wrong_signature,
                wrong_scope,
                wrong_root,
                wrong_attempt,
                wrong_purpose,
                wrong_ordinal,
            ] {
                assert!(
                    state
                        .values
                        .validate_resource_ordinary_attempt(forged)
                        .is_err()
                );
                state
                    .values
                    .validate_resource_ordinary_attempt(owner)
                    .unwrap();
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(complete(auth, attempt, 0).selected_kind, 0);
        assert_empty(auth);
    });
}

#[test]
fn malformed_ordinary_completion_cannot_seal_or_replace_the_running_channel() {
    context(|auth| {
        install(auth, &[], 1);
        let (attempt, _, _, _) = start(auth, ResourcePurpose::Runtime);
        auth.with_state(|_, state| {
            let owner = state
                .resource_state
                .as_ref()
                .unwrap()
                .attempt
                .as_ref()
                .unwrap()
                .ordinary_owner;
            let base = ResourceOrdinaryCompletion {
                owner,
                completion: NativeResourceCompletion {
                    attempt: owner.attempt.raw(),
                    body_status: 0,
                    cleanup_status: 0,
                    selected_kind: 0,
                    reserved: 0,
                },
                diagnostic_ready: true,
            };
            let mut wrong_attempt = base;
            wrong_attempt.completion.attempt += 1;
            let mut wrong_reserved = base;
            wrong_reserved.completion.reserved = 1;
            let mut wrong_channel = base;
            wrong_channel.completion.selected_kind = 4;
            let mut wrong_owner = base;
            wrong_owner.owner.ordinal += 1;
            for forged in [wrong_attempt, wrong_reserved, wrong_channel, wrong_owner] {
                assert!(state.values.seal_resource_ordinary_attempt(forged).is_err());
                state
                    .values
                    .validate_resource_ordinary_attempt(owner)
                    .unwrap();
            }
            assert!(state.resource_state.as_ref().unwrap().completed.is_empty());
            Ok(())
        })
        .unwrap();
        complete(auth, attempt, 0);
        assert_empty(auth);
    });
}

#[test]
fn sealed_ordinary_predecessor_requires_the_exact_saved_completion_and_ordinal() {
    context(|auth| {
        install(auth, &[], 2);
        let (attempt, _, _, _) = start(auth, ResourcePurpose::Runtime);
        let first = complete(auth, attempt, 0);
        auth.with_state(|_, state| {
            let resource = state.resource_state.as_ref().unwrap();
            let previous = resource.completed.last().unwrap().ordinary;
            let owner = previous.owner;
            assert!(
                state
                    .values
                    .validate_resource_ordinary_attempt(owner)
                    .is_err()
            );
            assert!(
                state
                    .values
                    .seal_resource_ordinary_attempt(previous)
                    .is_err()
            );
            let mut next = owner;
            next.attempt = ResourceHandleId::new(owner.attempt.raw() + 10).unwrap();
            next.root = ResourceHandleId::new(owner.root.raw() + 10).unwrap();
            next.ordinal += 1;
            let mut wrong_saved = previous;
            wrong_saved.completion.body_status = 1;
            assert!(
                state
                    .values
                    .prepare_resource_ordinary_attempt(next, Some(wrong_saved))
                    .is_err()
            );
            let mut wrong_ordinal = next;
            wrong_ordinal.ordinal += 1;
            assert!(
                state
                    .values
                    .prepare_resource_ordinary_attempt(wrong_ordinal, Some(previous))
                    .is_err()
            );
            let mut foreign = next;
            foreign.context.token = foreign.context.token.wrapping_add(1);
            assert!(
                state
                    .values
                    .prepare_resource_ordinary_attempt(foreign, Some(previous))
                    .is_err()
            );
            let mut wrong_entry = next;
            wrong_entry.entry.signature += 1;
            assert!(
                state
                    .values
                    .prepare_resource_ordinary_attempt(wrong_entry, Some(previous))
                    .is_err()
            );
            assert_eq!(resource.completed_attempt(attempt)?, first);
            assert!(resource.completed_messages(attempt)?.1.is_empty());
            Ok(())
        })
        .unwrap();
        let (second, _, _, _) = start(auth, ResourcePurpose::Runtime);
        assert!(second.raw() > attempt.raw());
        complete(auth, second, 0);
        assert_empty(auth);
    });
}

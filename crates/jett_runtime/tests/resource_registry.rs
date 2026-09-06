use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};

use jett_runtime::{AuthorityProvenance, ResourceRegistry, ResourceTypeId};

#[test]
fn explicit_close_runs_the_finalizer_exactly_once() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let finalizer_events = Arc::clone(&events);
    let mut registry = ResourceRegistry::new();
    let resource_type = ResourceTypeId::new(7);
    let authority = AuthorityProvenance::new(11, 13);

    let key = registry
        .insert(
            resource_type,
            String::from("socket-1"),
            authority,
            move |payload| {
                finalizer_events.lock().unwrap().push(payload);
            },
        )
        .expect("fresh registry should accept resources");

    registry
        .close(key, resource_type)
        .expect("close should succeed");
    assert_eq!(&*events.lock().unwrap(), &[String::from("socket-1")]);
    assert_eq!(registry.live_count(), 0);
}

#[test]
fn validates_carrier_identity_before_provider_access() {
    let resource_type = ResourceTypeId::new(7);
    let other_type = ResourceTypeId::new(8);
    let authority = AuthorityProvenance::new(11, 13);
    let wrong_authority = AuthorityProvenance::new(11, 99);
    let mut registry = ResourceRegistry::new();
    let key = registry
        .insert(resource_type, 41_u64, authority, |_| {})
        .expect("insert should succeed");

    assert_eq!(
        registry.access(key, resource_type, &wrong_authority, |value: &mut u64| {
            *value
        }),
        Err(jett_runtime::RegistryError::AuthorityMismatch)
    );
    assert_eq!(
        registry.access(key, other_type, &authority, |value: &mut u64| *value),
        Err(jett_runtime::RegistryError::WrongType)
    );
    let mut foreign_registry = ResourceRegistry::new();
    assert_eq!(
        foreign_registry.access(key, resource_type, &authority, |value: &mut u64| *value),
        Err(jett_runtime::RegistryError::WrongContext)
    );

    registry
        .close(key, resource_type)
        .expect("close should succeed");
    let replacement = registry
        .insert(resource_type, 42_u64, authority, |_| {})
        .expect("retired slot should be reusable");
    assert_eq!(
        registry.access(key, resource_type, &authority, |value: &mut u64| *value),
        Err(jett_runtime::RegistryError::StaleGeneration)
    );
    assert_eq!(
        registry.access(replacement, resource_type, &authority, |value: &mut u64| {
            *value
        }),
        Ok(42)
    );
}

#[test]
fn cancellation_detaches_pending_work_and_rejects_late_completion() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let resource_type = ResourceTypeId::new(7);
    let authority = AuthorityProvenance::new(11, 13);
    let mut registry = ResourceRegistry::new();
    let key = registry
        .insert(resource_type, (), authority, |_| {})
        .expect("insert should succeed");

    let first_events = Arc::clone(&events);
    let first = registry
        .begin_pending(key, resource_type, &authority, move || {
            first_events.lock().unwrap().push("cancel-first");
        })
        .expect("first operation should start");
    registry
        .cancel_pending(first)
        .expect("pending operation should cancel");
    assert_eq!(&*events.lock().unwrap(), &["cancel-first"]);
    assert_eq!(
        registry.live_count(),
        1,
        "borrowed cancellation keeps the resource live"
    );

    let second_events = Arc::clone(&events);
    let second = registry
        .begin_pending(key, resource_type, &authority, move || {
            second_events.lock().unwrap().push("cancel-second");
        })
        .expect("second operation should start");
    assert_eq!(
        registry.complete_pending(first),
        Err(jett_runtime::RegistryError::StaleOperation)
    );
    registry
        .complete_pending(second)
        .expect("current operation should complete");
    assert_eq!(&*events.lock().unwrap(), &["cancel-first"]);
}

#[test]
fn shutdown_finalizes_live_resources_in_reverse_creation_order() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let resource_type = ResourceTypeId::new(7);
    let authority = AuthorityProvenance::new(11, 13);
    let mut registry = ResourceRegistry::new();

    let first_finalizer_events = Arc::clone(&events);
    let first = registry
        .insert(resource_type, "first", authority, move |payload| {
            first_finalizer_events.lock().unwrap().push(payload);
        })
        .expect("first insert should succeed");
    let second_finalizer_events = Arc::clone(&events);
    registry
        .insert(resource_type, "second", authority, move |payload| {
            second_finalizer_events.lock().unwrap().push(payload);
        })
        .expect("second insert should succeed");
    let detach_events = Arc::clone(&events);
    registry
        .begin_pending(first, resource_type, &authority, move || {
            detach_events.lock().unwrap().push("detach-first");
        })
        .expect("operation should start");

    registry.shutdown();

    assert_eq!(
        &*events.lock().unwrap(),
        &["second", "detach-first", "first"]
    );
    assert_eq!(registry.live_count(), 0);
    assert_eq!(
        registry.insert(resource_type, "late", authority, |_| {}),
        Err(jett_runtime::RegistryError::ShuttingDown)
    );
}

#[test]
fn dropping_a_registry_finalizes_its_live_resources() {
    let events = Arc::new(Mutex::new(Vec::new()));
    {
        let mut registry = ResourceRegistry::new();
        let finalizer_events = Arc::clone(&events);
        registry
            .insert(
                ResourceTypeId::new(7),
                "resource",
                AuthorityProvenance::new(11, 13),
                move |payload| finalizer_events.lock().unwrap().push(payload),
            )
            .expect("insert should succeed");
    }

    assert_eq!(&*events.lock().unwrap(), &["resource"]);
}

#[test]
fn failing_cleanup_still_detaches_and_finalizes_every_resource_once() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut registry = ResourceRegistry::new();
    let resource_type = ResourceTypeId::new(7);
    let authority = AuthorityProvenance::new(11, 13);
    for name in ["first", "second"] {
        let finalizer_events = Arc::clone(&events);
        let key = registry
            .insert(resource_type, name, authority, move |name| {
                finalizer_events.lock().unwrap().push(name);
                if name == "second" {
                    panic!("provider finalizer failed");
                }
            })
            .unwrap();
        if name == "second" {
            registry
                .begin_pending(key, resource_type, &authority, || {
                    panic!("provider detach failed");
                })
                .unwrap();
        }
    }
    assert!(catch_unwind(AssertUnwindSafe(|| registry.shutdown())).is_err());
    assert_eq!(registry.live_count(), 0);
    registry.shutdown();
    drop(registry);
    assert_eq!(&*events.lock().unwrap(), &["second", "first"]);
}

#[test]
fn rejected_insert_finalizes_the_payload_it_consumed() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut registry = ResourceRegistry::new();
    registry.shutdown();
    let finalized = Arc::clone(&events);
    assert_eq!(
        registry.insert(
            ResourceTypeId::new(1),
            "rejected",
            AuthorityProvenance::new(1, 1),
            move |payload| {
                finalized.lock().unwrap().push(payload);
            }
        ),
        Err(jett_runtime::RegistryError::ShuttingDown)
    );
    assert_eq!(&*events.lock().unwrap(), &["rejected"]);
}

#[test]
fn cleanup_panics_do_not_abort_an_existing_unwind() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let finalized = Arc::clone(&events);
    assert!(
        catch_unwind(move || {
            let mut registry = ResourceRegistry::new();
            registry
                .insert(
                    ResourceTypeId::new(1),
                    (),
                    AuthorityProvenance::new(1, 1),
                    move |_| {
                        finalized.lock().unwrap().push("finalized");
                        panic!("cleanup failed");
                    },
                )
                .unwrap();
            panic!("original failure");
        })
        .is_err()
    );
    assert_eq!(&*events.lock().unwrap(), &["finalized"]);
}

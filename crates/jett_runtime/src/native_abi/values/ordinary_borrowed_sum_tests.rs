//! Nonconsuming ordinary sum reads keep the original shell and payload owners.
use super::*;
use std::mem::MaybeUninit;

struct BorrowContext {
    record: Box<JettRuntimeContextV1>,
    live: bool,
}

impl BorrowContext {
    fn new() -> Self {
        let mut record = Box::new(JettRuntimeContextV1::retired());
        let mut result = MaybeUninit::uninit();
        assert_eq!(
            unsafe { jett_rt_v1_context_create(1, &mut *record, result.as_mut_ptr()) },
            JettRuntimeStatusV1::OK
        );
        Self { record, live: true }
    }

    fn pointer(&self) -> *const JettRuntimeContextV1 {
        &*self.record
    }

    fn values<R>(&self, inspect: impl FnOnce(&mut NativeValues) -> R) -> R {
        let lease = acquire_context(context_key(self.pointer()).unwrap()).unwrap();
        let mut state = lock_unpoisoned(&lease.entry.state);
        inspect(&mut state.as_mut().unwrap().values)
    }

    fn retire(&mut self) {
        assert!(self.live);
        let mut result = MaybeUninit::uninit();
        assert_eq!(
            unsafe { jett_rt_v1_context_destroy(&mut *self.record, result.as_mut_ptr()) },
            JettRuntimeStatusV1::OK
        );
        self.live = false;
    }
}

impl Drop for BorrowContext {
    fn drop(&mut self) {
        if self.live {
            self.retire();
        }
    }
}

fn scalar_sum(context: &BorrowContext, tag: u32, bits: u64, depth: u64) -> u64 {
    let sum = unsafe { jett_rt_v1_sum_new_scalar_task(context.pointer(), tag, bits, depth) };
    assert_ne!(sum, 0);
    sum
}

fn drop_sum(context: &BorrowContext, sum: u64) {
    assert_eq!(unsafe { jett_rt_v1_value_drop(context.pointer(), sum) }, 0);
}

#[test]
fn ordinary_borrowed_sum_leaf_schema_matches_the_checked_native_call() {
    assert_eq!(
        NativeLeaf::SumPayloadBorrow.symbol(),
        "jett_rt_v1_sum_payload_borrow"
    );
    assert_eq!(
        NativeLeaf::SumPayloadBorrow.parameters(),
        &[AbiScalar::Pointer, AbiScalar::I64, AbiScalar::I32]
    );
    assert_eq!(NativeLeaf::SumPayloadBorrow.result(), AbiScalar::I64);
    let _: unsafe extern "C" fn(*const JettRuntimeContextV1, u64, u32) -> u64 =
        jett_rt_v1_sum_payload_borrow;
}

#[test]
fn ordinary_borrowed_sum_repeated_nested_reads_keep_identity_without_allocating() {
    let context = BorrowContext::new();
    let pointer = context.pointer();
    let bytes = unsafe { jett_rt_v1_bytes_new(pointer) };
    let list = unsafe { jett_rt_v1_list_new(pointer, 1) };
    assert_eq!(
        unsafe { jett_rt_v1_list_append(pointer, list, bytes) },
        list
    );
    let inner = unsafe { jett_rt_v1_sum_new(pointer, SUM_SUCCESS, list, 1) };
    let outer = unsafe { jett_rt_v1_sum_new(pointer, SUM_SUCCESS, inner, 1) };
    context.values(|values| values.allocation_budget = Some(0));
    for _ in 0..3 {
        assert_eq!(
            unsafe { jett_rt_v1_sum_payload_borrow(pointer, outer, SUM_SUCCESS) },
            inner
        );
        assert_eq!(
            unsafe { jett_rt_v1_sum_payload_borrow(pointer, inner, SUM_SUCCESS) },
            list
        );
    }
    context.values(|values| {
        assert_eq!(values.allocation_budget, Some(0));
        assert_eq!((values.sums_created, values.sums_destroyed), (2, 0));
        assert_eq!((values.lists_created, values.lists_destroyed), (1, 0));
        assert_eq!((values.bytes_created, values.bytes_destroyed), (1, 0));
        assert!(values.sums[&outer].owned && values.sums[&inner].owned);
        assert_eq!(values.lists[&list].elements, vec![Some(bytes)]);
        assert!(values.bytes.contains_key(&bytes));
        assert!(values.failure.is_none());
        assert!(values.dynamic_failure_message.is_none());
    });
    drop_sum(&context, outer);
    context.values(|values| {
        assert_eq!((values.sums_created, values.sums_destroyed), (2, 2));
        assert_eq!((values.lists_created, values.lists_destroyed), (1, 1));
        assert_eq!((values.bytes_created, values.bytes_destroyed), (1, 1));
        assert!(values.is_empty());
        assert!(!values.cleanup_failed);
    });
}

#[test]
fn ordinary_borrowed_sum_failure_string_needs_an_explicit_owned_reference() {
    let context = BorrowContext::new();
    let text = context.values(|values| {
        let text = values.insert("failure payload".into()).unwrap();
        values.strings.get_mut(&text).unwrap().pending_depth = 3;
        text
    });
    let sum = unsafe { jett_rt_v1_sum_new(context.pointer(), SUM_FAILURE, text, 1) };
    context.values(|values| values.allocation_budget = Some(0));
    for _ in 0..3 {
        assert_eq!(
            unsafe { jett_rt_v1_sum_payload_borrow(context.pointer(), sum, SUM_FAILURE) },
            text
        );
    }
    context.values(|values| {
        assert_eq!(values.strings[&text].references, 1);
        assert_eq!(values.strings[&text].pending_depth, 3);
        assert_eq!(values.sums[&sum].payload_pending_depth, 0);
        assert_eq!(values.allocation_budget, Some(0));
    });
    let retained = unsafe { jett_rt_v1_string_retain(context.pointer(), text) };
    assert_eq!(retained, text);
    context.values(|values| assert_eq!(values.strings[&text].references, 2));
    drop_sum(&context, sum);
    context.values(|values| {
        assert_eq!(values.strings[&text].references, 1);
        assert_eq!(values.strings[&text].pending_depth, 3);
        assert_eq!(values.text(text), Ok("failure payload"));
    });
    assert_eq!(
        unsafe { jett_rt_v1_string_release(context.pointer(), retained) },
        0
    );
    context.values(|values| assert!(values.is_empty()));
}

#[test]
fn ordinary_borrowed_sum_scalar_bits_and_pending_depth_are_not_flattened() {
    for tag in [SUM_FAILURE, SUM_SUCCESS] {
        for (bits, depth) in [(0, 0), (u64::MAX, 1), (7, 2), (0, u64::MAX)] {
            let context = BorrowContext::new();
            let sum = scalar_sum(&context, tag, bits, depth);
            context.values(|values| values.allocation_budget = Some(0));
            assert_eq!(
                unsafe { jett_rt_v1_sum_payload_borrow(context.pointer(), sum, tag) },
                bits
            );
            assert_eq!(
                unsafe { jett_rt_v1_sum_payload_pending_depth(context.pointer(), sum) },
                depth
            );
            assert_eq!(unsafe { jett_rt_v1_value_status(context.pointer()) }, 0);
            context.values(|values| {
                let shell = &values.sums[&sum];
                assert_eq!((shell.tag, shell.bits, shell.owned), (tag, bits, false));
                assert_eq!(
                    (shell.pending_depth, shell.payload_pending_depth),
                    (0, depth)
                );
                assert_eq!((values.sums_created, values.sums_destroyed), (1, 0));
                assert_eq!(values.allocation_budget, Some(0));
            });
            drop_sum(&context, sum);
            context.values(|values| assert!(values.is_empty()));
        }
    }
}

#[test]
fn ordinary_borrowed_sum_ready_shell_keeps_pending_list_and_element_depth() {
    let context = BorrowContext::new();
    let pointer = context.pointer();
    let list = unsafe { jett_rt_v1_list_new(pointer, 0) };
    assert_eq!(
        unsafe { jett_rt_v1_list_append_scalar_task(pointer, list, 5, 2) },
        list
    );
    context.values(|values| values.lists.get_mut(&list).unwrap().pending_depth = 3);
    let sum = unsafe { jett_rt_v1_sum_new(pointer, SUM_SUCCESS, list, 1) };
    context.values(|values| values.allocation_budget = Some(0));
    assert_eq!(
        unsafe { jett_rt_v1_sum_payload_borrow(pointer, sum, SUM_SUCCESS) },
        list
    );
    assert_eq!(
        unsafe { jett_rt_v1_list_element_pending_depth(pointer, list, 0) },
        2
    );
    context.values(|values| {
        assert_eq!(values.lists[&list].pending_depth, 3);
        assert_eq!(values.lists[&list].element_pending_depths.get(&0), Some(&2));
        assert_eq!(values.sums[&sum].pending_depth, 0);
        assert_eq!(values.sums[&sum].payload_pending_depth, 0);
        assert_eq!(values.allocation_budget, Some(0));
        assert!(values.failure.is_none());
    });
    drop_sum(&context, sum);
    context.values(|values| assert!(values.is_empty()));
}

#[test]
fn ordinary_borrowed_sum_wrong_tags_and_outer_pending_keep_the_owned_shell() {
    for (tag, selected, outer_depth) in [
        (SUM_SUCCESS, SUM_FAILURE, 0),
        (SUM_FAILURE, SUM_SUCCESS, 0),
        (SUM_SUCCESS, 2, 0),
        (SUM_SUCCESS, u32::MAX, 0),
        (SUM_SUCCESS, SUM_SUCCESS, 1),
        (SUM_FAILURE, SUM_FAILURE, 2),
        (SUM_SUCCESS, SUM_SUCCESS, u64::MAX),
    ] {
        let context = BorrowContext::new();
        let list = unsafe { jett_rt_v1_list_new(context.pointer(), 0) };
        let sum = unsafe { jett_rt_v1_sum_new(context.pointer(), tag, list, 1) };
        context.values(|values| {
            let shell = values.sums.get_mut(&sum).unwrap();
            shell.pending_depth = outer_depth;
            shell.payload_pending_depth = 7;
            values.allocation_budget = Some(0);
        });
        assert_eq!(
            unsafe { jett_rt_v1_sum_payload_borrow(context.pointer(), sum, selected) },
            0
        );
        assert_eq!(
            unsafe { jett_rt_v1_value_status(context.pointer()) },
            JettRuntimeStatusV1::INVALID_ARGUMENT.code()
        );
        context.values(|values| {
            let shell = &values.sums[&sum];
            assert_eq!((shell.tag, shell.bits, shell.owned), (tag, list, true));
            assert_eq!(
                (shell.pending_depth, shell.payload_pending_depth),
                (outer_depth, 7)
            );
            assert_eq!(values.failure, Some(INVALID_SUM));
            assert!(values.dynamic_failure_message.is_none());
            assert_eq!((values.sums_created, values.sums_destroyed), (1, 0));
            assert_eq!((values.lists_created, values.lists_destroyed), (1, 0));
            assert_eq!(values.allocation_budget, Some(0));
        });
        drop_sum(&context, sum);
        context.values(|values| {
            assert!(values.is_empty());
            assert!(!values.cleanup_failed);
        });
    }
}

#[test]
fn ordinary_borrowed_sum_foreign_stale_and_non_sum_ids_keep_other_owners() {
    let foreign_context = BorrowContext::new();
    let foreign = scalar_sum(&foreign_context, SUM_SUCCESS, 41, 0);
    for invalid_kind in 0..5 {
        let context = BorrowContext::new();
        let list = unsafe { jett_rt_v1_list_new(context.pointer(), 0) };
        let sum = unsafe { jett_rt_v1_sum_new(context.pointer(), SUM_SUCCESS, list, 1) };
        let stale = scalar_sum(&context, SUM_SUCCESS, 43, 0);
        drop_sum(&context, stale);
        let invalid = match invalid_kind {
            0 => 0,
            1 => u64::MAX,
            2 => foreign,
            3 => list,
            4 => stale,
            _ => unreachable!(),
        };
        assert_eq!(
            unsafe { jett_rt_v1_sum_payload_borrow(context.pointer(), invalid, SUM_SUCCESS) },
            0
        );
        // This leaf is not a cleanup operation: terminal failure inhibits it.
        assert_eq!(
            unsafe { jett_rt_v1_sum_payload_borrow(context.pointer(), sum, SUM_SUCCESS) },
            0
        );
        context.values(|values| {
            assert_eq!(values.failure, Some(INVALID_SUM));
            assert_eq!(values.sums[&sum].bits, list);
            assert!(values.sums[&sum].owned);
            assert!(values.lists.contains_key(&list));
            assert_eq!((values.sums_created, values.sums_destroyed), (2, 1));
            assert_eq!((values.lists_created, values.lists_destroyed), (1, 0));
        });
        drop_sum(&context, sum);
        context.values(|values| {
            assert!(values.is_empty());
            assert!(!values.cleanup_failed);
        });
    }
    assert_eq!(
        unsafe { jett_rt_v1_sum_payload_borrow(foreign_context.pointer(), foreign, SUM_SUCCESS) },
        41
    );
    drop_sum(&foreign_context, foreign);
    foreign_context.values(|values| assert!(values.is_empty()));
}

#[test]
fn ordinary_borrowed_sum_null_copied_mismatched_and_retired_contexts_are_inert() {
    let mut context = BorrowContext::new();
    let sum = scalar_sum(&context, SUM_SUCCESS, 47, 0);
    let copied = JettRuntimeContextV1 {
        token: context.record.token,
        abi_version: context.record.abi_version,
        state_tag: context.record.state_tag,
    };
    let retired = JettRuntimeContextV1::retired();
    for pointer in [ptr::null(), &copied, &retired] {
        assert_eq!(
            unsafe { jett_rt_v1_sum_payload_borrow(pointer, sum, SUM_SUCCESS) },
            0
        );
    }
    context.record.abi_version = 2;
    assert_eq!(
        unsafe { jett_rt_v1_sum_payload_borrow(context.pointer(), sum, SUM_SUCCESS) },
        0
    );
    context.record.abi_version = JETT_RUNTIME_ABI_VERSION_V1;
    assert_eq!(
        unsafe { jett_rt_v1_sum_payload_borrow(context.pointer(), sum, SUM_SUCCESS) },
        47
    );
    context.values(|values| {
        assert!(values.failure.is_none());
        assert_eq!((values.sums_created, values.sums_destroyed), (1, 0));
    });
    drop_sum(&context, sum);
    context.values(|values| assert!(values.is_empty()));
    context.retire();
    assert_eq!(
        unsafe { jett_rt_v1_sum_payload_borrow(context.pointer(), sum, SUM_SUCCESS) },
        0
    );
}

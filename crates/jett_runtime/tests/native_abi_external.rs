//! Exercise the runtime through independently declared C-compatible records and
//! exact link names rather than through Rust item references.

use std::mem::{MaybeUninit, align_of, offset_of, size_of};
use std::ptr;

#[repr(C)]
struct ExternalContextV1 {
    token: u64,
    abi_version: u32,
    state_tag: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ExternalStringSliceV1 {
    data: *const u8,
    byte_length: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ExternalResultV1 {
    status: u32,
    message: ExternalStringSliceV1,
}

unsafe extern "C" {
    #[link_name = "jett_rt_v1_abi_version"]
    static EXTERNAL_ABI_VERSION: u32;

    #[link_name = "jett_rt_v1_check_abi"]
    fn external_check_abi(expected_version: u32, out_result: *mut ExternalResultV1) -> u32;

    #[link_name = "jett_rt_v1_context_create"]
    fn external_context_create(
        expected_version: u32,
        out_context: *mut ExternalContextV1,
        out_result: *mut ExternalResultV1,
    ) -> u32;

    #[link_name = "jett_rt_v1_context_destroy"]
    fn external_context_destroy(
        context: *mut ExternalContextV1,
        out_result: *mut ExternalResultV1,
    ) -> u32;

    #[link_name = "jett_rt_v1_stdout_write"]
    fn external_stdout_write(
        context: *const ExternalContextV1,
        string: *const ExternalStringSliceV1,
        out_result: *mut ExternalResultV1,
    ) -> u32;
}

fn call_result(call: impl FnOnce(*mut ExternalResultV1) -> u32) -> (u32, ExternalResultV1) {
    let mut result = MaybeUninit::uninit();
    let status = call(result.as_mut_ptr());
    (status, unsafe { result.assume_init() })
}

#[test]
fn independently_declared_ms_c_abi_links_and_runs() {
    // Force the Rust dependency onto the link line while all ABI operations
    // below are resolved exclusively through their exact external symbols.
    drop(jett_runtime::ResourceRegistry::new());

    assert_eq!(size_of::<ExternalContextV1>(), 16);
    assert_eq!(align_of::<ExternalContextV1>(), 8);
    assert_eq!(offset_of!(ExternalContextV1, token), 0);
    assert_eq!(offset_of!(ExternalContextV1, abi_version), 8);
    assert_eq!(offset_of!(ExternalContextV1, state_tag), 12);
    assert_eq!(size_of::<ExternalStringSliceV1>(), 16);
    assert_eq!(align_of::<ExternalStringSliceV1>(), 8);
    assert_eq!(size_of::<ExternalResultV1>(), 24);
    assert_eq!(align_of::<ExternalResultV1>(), 8);
    assert_eq!(offset_of!(ExternalResultV1, status), 0);
    assert_eq!(offset_of!(ExternalResultV1, message), 8);
    assert_eq!(unsafe { EXTERNAL_ABI_VERSION }, 1);

    let (status, checked) = call_result(|out| unsafe { external_check_abi(1, out) });
    assert_eq!(status, 0);
    assert_eq!(checked.status, status);

    let mut context = MaybeUninit::<ExternalContextV1>::uninit();
    let (status, created) =
        call_result(|out| unsafe { external_context_create(1, context.as_mut_ptr(), out) });
    assert_eq!(status, 0);
    assert_eq!(created.status, status);
    // Keep using the exact storage address passed to create. Moving the
    // record is deliberately rejected by the ABI's owner-address binding.
    let context = unsafe { &mut *context.as_mut_ptr() };

    let empty = ExternalStringSliceV1 {
        data: ptr::null(),
        byte_length: 0,
    };
    let (status, wrote) = call_result(|out| unsafe { external_stdout_write(context, &empty, out) });
    assert_eq!(status, 0);
    assert_eq!(wrote.status, status);

    let (status, destroyed) = call_result(|out| unsafe { external_context_destroy(context, out) });
    assert_eq!(status, 0);
    assert_eq!(destroyed.status, status);

    let (status, repeated) = call_result(|out| unsafe { external_context_destroy(context, out) });
    assert_eq!(status, 3);
    assert_eq!(repeated.status, status);
}

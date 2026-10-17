//! Metadata-only bridge from exact Resource Descriptor rows to frozen graph fields.
use super::*;

/// Its private constructor seals the original scalar descriptor to one installation.
/// This record contains no Scope, grant, owner, loan, callable code or environment.
#[derive(Clone)]
pub(in crate::native_abi) struct CheckedGraphHookProof {
    context: ContextRegistryKey,
    program: ResourceHandleId,
    layout: Arc<RegisteredNativeLayout>,
    descriptor: ResourceHandleId,
    descriptor_operation: u32,
    hook: u32,
    signature: u32,
}

impl CheckedGraphHookProof {
    pub(in crate::native_abi) fn matches_schema(
        &self,
        operation: u32,
        hook: u32,
        signature: u32,
    ) -> bool {
        self.descriptor_operation == operation && self.hook == hook && self.signature == signature
    }
}

impl NativeResourceState {
    pub(super) fn graph_descriptor_box(
        &self,
        ordinary: &mut values::NativeValues,
        ticket: u64,
        descriptor_operation: u32,
        descriptor: ResourceHandleId,
    ) -> ResourceResult<u64> {
        self.running(ordinary)?;
        let hook = match self.operation(descriptor_operation)? {
            NativeOperation::Descriptor { hook } => *hook,
            _ => return Err(NativeResourceError::WrongOperation),
        };
        let signature = self
            .layout
            .hooks()
            .get(hook as usize)
            .ok_or(NativeResourceError::WrongOperation)?
            .signature();
        match self.handles.get(&descriptor) {
            Some(NativeResourceEntry::Descriptor {
                hook: actual,
                signature: actual_signature,
            }) if *actual == hook && *actual_signature == signature => {}
            _ => return Err(NativeResourceError::WrongFamily),
        }
        let proof = CheckedGraphHookProof {
            context: self.context,
            program: self.program_handle,
            layout: Arc::clone(&self.layout),
            descriptor,
            descriptor_operation,
            hook,
            signature,
        };
        ordinary
            .mint_checked_graph_hook(ticket, proof)
            .map_err(ordinary_error)
    }

    /// Used only by the existing exact indirect Resource row before its frame checks.
    /// The returned ID is the original, installation-lived scalar Descriptor family.
    pub(super) fn graph_descriptor_scalar(
        &self,
        ordinary: &values::NativeValues,
        operation: u32,
        boxed: ResourceHandleId,
    ) -> ResourceResult<ResourceHandleId> {
        self.validate_installation()?;
        let (hook, signature) = match self.operation(operation)? {
            NativeOperation::InvokeDescriptor {
                hook, signature, ..
            } => (*hook, *signature),
            _ => return Err(NativeResourceError::WrongOperation),
        };
        let proof = ordinary
            .published_checked_graph_hook(boxed.raw())
            .map_err(ordinary_error)?;
        if proof.context != self.context
            || proof.program != self.program_handle
            || !Arc::ptr_eq(&proof.layout, &self.layout)
            || proof.hook != hook
            || proof.signature != signature
        {
            return Err(NativeResourceError::WrongFamily);
        }
        match self.handles.get(&proof.descriptor) {
            Some(NativeResourceEntry::Descriptor {
                hook: actual,
                signature: actual_signature,
            }) if *actual == hook && *actual_signature == signature => Ok(proof.descriptor),
            _ => Err(NativeResourceError::WrongFamily),
        }
    }
}

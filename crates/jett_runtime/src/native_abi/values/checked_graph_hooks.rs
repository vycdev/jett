//! Distinct ordinary-owned boxes for checked Resource hook metadata.
use super::*;
use crate::native_abi::resource::CheckedGraphHookProof;

#[derive(Clone, Copy)]
pub(super) enum Phase {
    Factory(u64),
    Published,
}

#[derive(Clone)]
pub(super) struct CheckedGraphHook {
    pub(super) proof: CheckedGraphHookProof,
    pub(super) phase: Phase,
    pub(super) pending_depth: u64,
}

impl CheckedGraphHook {
    pub(super) fn matches_factory(
        &self,
        ticket: u64,
        operation: u32,
        hook: u32,
        signature: u32,
    ) -> bool {
        matches!(self.phase, Phase::Factory(actual) if actual == ticket)
            && self.pending_depth == 0
            && self.proof.matches_schema(operation, hook, signature)
    }
    pub(super) fn publish(&mut self) {
        self.phase = Phase::Published;
    }
}

impl NativeValues {
    fn insert_checked_graph_hook(&mut self, value: CheckedGraphHook) -> LeafResult<u64> {
        #[cfg(test)]
        self.allocation_checkpoint()?;
        let created = self
            .checked_graph_hooks_created
            .checked_add(1)
            .ok_or(EXHAUSTED)?;
        self.checked_graph_hooks
            .try_reserve(1)
            .map_err(|_| EXHAUSTED)?;
        let id = next_identity()?;
        self.checked_graph_hooks.insert(id, value);
        self.checked_graph_hooks_created = created;
        Ok(id)
    }

    pub(in crate::native_abi) fn mint_checked_graph_hook(
        &mut self,
        ticket: u64,
        proof: CheckedGraphHookProof,
    ) -> LeafResult<u64> {
        if !self.actor_graph_imports.contains_key(&ticket) {
            return Err(actor_graph_import::INVALID_IMPORT);
        }
        self.insert_checked_graph_hook(CheckedGraphHook {
            proof,
            phase: Phase::Factory(ticket),
            pending_depth: 0,
        })
    }

    pub(in crate::native_abi) fn published_checked_graph_hook(
        &self,
        id: u64,
    ) -> LeafResult<&CheckedGraphHookProof> {
        let value = self.checked_graph_hooks.get(&id).ok_or(INVALID_FUNCTION)?;
        if !matches!(value.phase, Phase::Published) || value.pending_depth != 0 {
            return Err(INVALID_FUNCTION);
        }
        Ok(&value.proof)
    }

    pub(super) fn clone_checked_graph_hook(
        &mut self,
        id: u64,
        depth: Option<u64>,
    ) -> LeafResult<u64> {
        let mut value = self
            .checked_graph_hooks
            .get(&id)
            .ok_or(INVALID_FUNCTION)?
            .clone();
        if let Some(depth) = depth {
            value.pending_depth = depth;
        }
        self.insert_checked_graph_hook(value)
    }

    pub(in crate::native_abi) fn rollback_graph_hook_failure(
        &mut self,
        error: (JettRuntimeStatusV1, &'static [u8]),
    ) {
        self.record_ordinary_failure(error);
        if let Err(cleanup) = self.abort_unfinished_actor_graphs() {
            self.cleanup_failed = true;
            self.record_ordinary_failure(cleanup);
        }
    }
}

//! Private unit-test faults at real fallible ordinary-entry reservation sites.
//! This module, its instance state and every checkpoint are absent without cfg(test).
use super::{NativeResourceError, ResourceResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::native_abi) enum ReservationSite {
    BeginActiveFrames,
    BeginHistory,
    OrdinaryChannel,
    ResourceDiagnostic,
    OrdinaryDiagnostic,
}

impl ReservationSite {
    fn index(self) -> usize {
        match self {
            Self::BeginActiveFrames => 0,
            Self::BeginHistory => 1,
            Self::OrdinaryChannel => 2,
            Self::ResourceDiagnostic => 3,
            Self::OrdinaryDiagnostic => 4,
        }
    }
}

#[derive(Default)]
pub(in crate::native_abi) struct ReservationFaults {
    armed: Option<ReservationSite>,
    growth_requests: [usize; 5],
    injected_refusals: [usize; 5],
}

impl ReservationFaults {
    pub(in crate::native_abi) fn arm(&mut self, site: ReservationSite) {
        assert!(
            self.armed.is_none(),
            "only one allocation fault may be pending per state"
        );
        self.armed = Some(site);
    }

    // A reserve that reuses capacity cannot produce an allocator failure. The
    // hook is directly before try_reserve and follows the guards preceding that
    // actual reservation. The ordinary-channel hook follows the exact predecessor proof.
    pub(in crate::native_abi) fn checkpoint(
        &mut self,
        site: ReservationSite,
        allocation_needed: bool,
    ) -> ResourceResult<()> {
        if !allocation_needed {
            return Ok(());
        }
        let index = site.index();
        self.growth_requests[index] += 1;
        if self.armed == Some(site) {
            self.armed = None;
            self.injected_refusals[index] += 1;
            return Err(NativeResourceError::Capacity);
        }
        Ok(())
    }

    pub(in crate::native_abi) fn snapshot(&self, site: ReservationSite) -> (usize, usize, bool) {
        let index = site.index();
        (
            self.growth_requests[index],
            self.injected_refusals[index],
            self.armed == Some(site),
        )
    }
}

//! Bounded, RAII-tracked CPU pixel buffers.
//!
//! The limit covers buffers explicitly allocated by the engine. It does not
//! claim to include graph metadata, decoder internals, OS allocations, or the
//! caller-owned output sink. A shared atomic counter allows concurrent plans
//! to honor the same managed-buffer budget in a future executor.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::error::Error;

#[derive(Debug)]
struct State {
    limit: usize,
    live: AtomicUsize,
    peak: AtomicUsize,
    allocations: AtomicUsize,
    allocated_bytes: AtomicUsize,
}

#[derive(Clone, Debug)]
pub(crate) struct MemoryBudget {
    state: Arc<State>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MemorySnapshot {
    pub limit: usize,
    pub live: usize,
    pub peak: usize,
    pub allocations: usize,
    pub allocated_bytes: usize,
}

impl MemoryBudget {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            state: Arc::new(State {
                limit,
                live: AtomicUsize::new(0),
                peak: AtomicUsize::new(0),
                allocations: AtomicUsize::new(0),
                allocated_bytes: AtomicUsize::new(0),
            }),
        }
    }

    pub(crate) fn snapshot(&self) -> MemorySnapshot {
        MemorySnapshot {
            limit: self.state.limit,
            live: self.state.live.load(Ordering::Acquire),
            peak: self.state.peak.load(Ordering::Acquire),
            allocations: self.state.allocations.load(Ordering::Relaxed),
            allocated_bytes: self.state.allocated_bytes.load(Ordering::Relaxed),
        }
    }

    pub(crate) fn allocate(&self, bytes: usize) -> Result<ManagedBytes, Error> {
        let mut live = self.state.live.load(Ordering::Acquire);
        loop {
            let available = self.state.limit.saturating_sub(live);
            if bytes > available {
                return Err(Error::BudgetExceeded {
                    requested: bytes,
                    available,
                });
            }
            let next = live.checked_add(bytes).ok_or(Error::GeometryOverflow)?;
            match self.state.live.compare_exchange_weak(
                live,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(observed) => live = observed,
            }
        }

        // The reservation, not the operating system allocation, defines
        // our managed-memory high-water mark. Count it before other threads
        // can release or reuse their reservations.
        self.state.peak.fetch_max(live + bytes, Ordering::AcqRel);

        // On allocation failure the reservation automatically releases its bytes.
        let reservation = Reservation {
            state: Arc::clone(&self.state),
            bytes,
        };
        let mut data = Vec::new();
        data.try_reserve_exact(bytes)
            .map_err(|_| Error::AllocationFailed)?;
        data.resize(bytes, 0);
        self.state.allocations.fetch_add(1, Ordering::Relaxed);
        self.state
            .allocated_bytes
            .fetch_add(bytes, Ordering::Relaxed);
        Ok(ManagedBytes { data, reservation })
    }
}

#[derive(Debug)]
struct Reservation {
    state: Arc<State>,
    bytes: usize,
}

impl Drop for Reservation {
    fn drop(&mut self) {
        self.state.live.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

pub(crate) struct ManagedBytes {
    // Released before `reservation` (field drop order) to avoid prematurely
    // advertising available memory while the allocation is still live.
    data: Vec<u8>,
    reservation: Reservation,
}

impl ManagedBytes {
    pub(crate) fn as_slice(&self) -> &[u8] {
        &self.data
    }

    pub(crate) fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

impl std::fmt::Debug for ManagedBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagedBytes")
            .field("len", &self.data.len())
            .field("reservation", &self.reservation.bytes)
            .finish()
    }
}

// SPDX-License-Identifier: MPL-2.0

//! RAM usage sensor, backed by `sysinfo`.

use sysinfo::{MemoryRefreshKind, RefreshKind, System};

/// Wraps `sysinfo::System` configured to only track memory stats.
pub struct Memory {
    sys: System,
}

impl Memory {
    pub fn new() -> Self {
        let sys = System::new_with_specifics(
            RefreshKind::nothing().with_memory(MemoryRefreshKind::everything()),
        );
        Self { sys }
    }

    /// Refresh memory stats and return `(used_bytes, total_bytes)`.
    pub fn refresh(&mut self) -> (u64, u64) {
        self.sys
            .refresh_specifics(RefreshKind::nothing().with_memory(MemoryRefreshKind::everything()));
        (self.sys.used_memory(), self.sys.total_memory())
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

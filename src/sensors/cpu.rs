// SPDX-License-Identifier: MPL-2.0

//! CPU usage sensor, backed by `sysinfo`.

use sysinfo::{CpuRefreshKind, RefreshKind, System};

/// Wraps `sysinfo::System` configured to only track CPU stats.
pub struct Cpu {
    sys: System,
}

impl Cpu {
    pub fn new() -> Self {
        let mut sys = System::new_with_specifics(
            RefreshKind::nothing().with_cpu(CpuRefreshKind::everything()),
        );
        // `sysinfo` needs two reads to produce a meaningful percentage: the
        // first call captures the initial state and the second compares the
        // current state against it. We do the priming here so the first
        // `refresh` already returns real numbers.
        sys.refresh_cpu_usage();
        Self { sys }
    }

    /// Refresh CPU stats and return the overall usage in percent.
    pub fn refresh(&mut self) -> f32 {
        self.sys
            .refresh_specifics(RefreshKind::nothing().with_cpu(CpuRefreshKind::everything()));
        self.sys.global_cpu_usage()
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

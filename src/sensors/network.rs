// SPDX-License-Identifier: MPL-2.0

//! Network throughput sensor, backed by `sysinfo::Networks`.
//!
//! `sysinfo::NetworkData::received()` and `transmitted()` return the number
//! of bytes transferred since the last refresh, so we simply divide by the
//! elapsed time to get a rate.

use std::time::Duration;

use sysinfo::Networks;

/// Interface prefixes we never want to surface in the auto-pick list.
const VIRTUAL_PREFIXES: &[&str] = &["lo", "docker", "br-", "veth", "virbr", "tun", "tap"];

/// Preferred physical interface prefixes, in priority order.
const PHYSICAL_PREFIXES: &[&str] = &["eth", "enp", "eno", "wlp", "wlan", "wlx"];

pub struct Network {
    nets: Networks,
}

impl Network {
    pub fn new() -> Self {
        let mut nets = Networks::new_with_refreshed_list();
        // Prime: read once so the first delta is accurate.
        nets.refresh(true);
        Self { nets }
    }

    /// Refresh network counters. Returns:
    /// `(rx_bytes_per_sec, tx_bytes_per_sec, selected_interface, available_interfaces)`.
    ///
    /// When `selection` is `Some`, the counters of that single interface are
    /// reported. Otherwise, the first non-virtual, non-loopback physical
    /// interface is auto-picked, falling back to the first interface that is
    /// not loopback.
    pub fn refresh_with(
        &mut self,
        since: Duration,
        selection: Option<&str>,
    ) -> (f64, f64, Option<String>, Vec<String>) {
        self.nets.refresh(true);
        let secs = since.as_secs_f64().max(0.001);

        let mut names: Vec<String> = self
            .nets
            .iter()
            .map(|(name, _)| name.to_string())
            .collect();
        names.sort();

        let chosen = selection
            .map(|s| s.to_string())
            .or_else(|| auto_pick(&names));

        let mut rx: u64 = 0;
        let mut tx: u64 = 0;
        if let Some(name) = chosen.as_deref() {
            if let Some(data) = self.nets.get(name) {
                rx = data.received();
                tx = data.transmitted();
            }
        }

        (rx as f64 / secs, tx as f64 / secs, chosen, names)
    }
}

impl Default for Network {
    fn default() -> Self {
        Self::new()
    }
}

/// Pick a sensible default interface. Prefers known physical prefixes, then
/// falls back to the first non-loopback, non-virtual interface. Returns
/// `None` if no candidate is found.
fn auto_pick(names: &[String]) -> Option<String> {
    for prefix in PHYSICAL_PREFIXES {
        if let Some(name) = names.iter().find(|n| n.starts_with(prefix)) {
            return Some(name.clone());
        }
    }
    names
        .iter()
        .find(|n| !is_virtual(n))
        .cloned()
}

fn is_virtual(name: &str) -> bool {
    VIRTUAL_PREFIXES.iter().any(|p| name.starts_with(p))
}

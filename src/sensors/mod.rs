// SPDX-License-Identifier: MPL-2.0

//! Sensor layer: keeps one handle per resource and exposes a uniform
//! `refresh_all` entry point that updates a cached [`SensorsSnapshot`].

mod cpu;
mod gpu;
mod memory;
mod network;

pub use cpu::Cpu;
pub use gpu::Gpus;
pub use memory::Memory;
pub use network::Network;

use std::time::Duration;

/// A single read of every sensor, taken at one point in time. Cheap to clone.
#[derive(Debug, Clone, Default)]
pub struct SensorsSnapshot {
    /// Overall CPU usage in percent (0.0–100.0).
    pub cpu_percent: f32,
    /// Used memory in bytes.
    pub memory_used: u64,
    /// Total memory in bytes.
    pub memory_total: u64,
    /// GPU utilization in percent (0.0–100.0), averaged across detected GPUs.
    pub gpu_percent: Option<f32>,
    /// Total VRAM used across all detected GPUs, in bytes.
    pub gpu_vram_used: Option<u64>,
    /// Total VRAM capacity across all detected GPUs, in bytes.
    pub gpu_vram_total: Option<u64>,
    /// Hottest GPU temperature in °C.
    pub gpu_temp_c: Option<f32>,
    /// Download throughput in bytes per second.
    pub net_rx_bps: f64,
    /// Upload throughput in bytes per second.
    pub net_tx_bps: f64,
    /// The network interface currently being monitored.
    pub net_interface: Option<String>,
    /// All interfaces that are currently considered monitorable.
    pub net_available: Vec<String>,
    /// Whether any GPU was detected at all.
    pub gpu_present: bool,
}

/// Aggregates handles for every sensor the applet monitors.
pub struct Sensors {
    cpu: Cpu,
    memory: Memory,
    gpu: Gpus,
    network: Network,
    /// Time elapsed between two `refresh_all` calls. Used by the network sensor
    /// to convert byte deltas to bytes-per-second.
    elapsed: Duration,
}

impl Sensors {
    /// Discover hardware and create handles for every sensor. Safe to call on
    /// any Linux system — sensors that aren't available simply stay empty.
    pub fn new() -> Self {
        Self {
            cpu: Cpu::new(),
            memory: Memory::new(),
            gpu: Gpus::discover(),
            network: Network::new(),
            elapsed: Duration::from_millis(0),
        }
    }

    /// Refresh every sensor in place. The given `since` is the time that has
    /// passed since the previous refresh (used for rate calculations).
    /// `network_interface` overrides the network sensor's auto-pick when
    /// `Some`; pass `None` to use auto-pick.
    pub fn refresh(
        &mut self,
        since: Duration,
        network_interface: Option<&str>,
    ) -> SensorsSnapshot {
        self.elapsed = since;

        let cpu_percent = self.cpu.refresh();
        let (memory_used, memory_total) = self.memory.refresh();
        let (rx_bps, tx_bps, interface, available) =
            self.network.refresh_with(since, network_interface);
        let (gpu_percent, vram_used, vram_total, gpu_temp, present) = self.gpu.refresh();

        SensorsSnapshot {
            cpu_percent,
            memory_used,
            memory_total,
            gpu_percent,
            gpu_vram_used: vram_used,
            gpu_vram_total: vram_total,
            gpu_temp_c: gpu_temp,
            net_rx_bps: rx_bps,
            net_tx_bps: tx_bps,
            net_interface: interface,
            net_available: available,
            gpu_present: present,
        }
    }
}

impl Default for Sensors {
    fn default() -> Self {
        Self::new()
    }
}

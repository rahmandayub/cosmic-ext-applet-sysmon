// SPDX-License-Identifier: MPL-2.0

use cosmic::cosmic_config::{self, cosmic_config_derive::CosmicConfigEntry, CosmicConfigEntry};

/// How often the applet refreshes sensor data, in milliseconds.
pub const DEFAULT_REFRESH_INTERVAL_MS: u64 = 2000;
/// Lower bound the user can pick for the refresh interval.
pub const MIN_REFRESH_INTERVAL_MS: u64 = 250;
/// Upper bound the user can pick for the refresh interval.
pub const MAX_REFRESH_INTERVAL_MS: u64 = 10_000;

#[derive(Debug, Clone, CosmicConfigEntry, PartialEq)]
#[version = 1]
pub struct Config {
    /// How often sensor data is refreshed, in milliseconds.
    pub refresh_interval_ms: u64,
    /// Show the CPU usage metric on the panel.
    pub show_cpu: bool,
    /// Show the memory usage metric on the panel.
    pub show_memory: bool,
    /// Show the GPU usage / VRAM / temperature metric on the panel.
    pub show_gpu: bool,
    /// Show the network download / upload metric on the panel.
    pub show_network: bool,
    /// Network interface to monitor. `None` = auto-pick the primary non-loopback
    /// interface.
    pub network_interface: Option<String>,
    /// CPU warning threshold percentage (0-100).
    pub cpu_warning_threshold: f32,
    /// CPU critical threshold percentage (0-100).
    pub cpu_critical_threshold: f32,
    /// RAM warning threshold percentage (0-100).
    pub ram_warning_threshold: f32,
    /// RAM critical threshold percentage (0-100).
    pub ram_critical_threshold: f32,
    /// GPU warning threshold percentage (0-100).
    pub gpu_warning_threshold: f32,
    /// GPU critical threshold percentage (0-100).
    pub gpu_critical_threshold: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            refresh_interval_ms: DEFAULT_REFRESH_INTERVAL_MS,
            show_cpu: true,
            show_memory: true,
            show_gpu: true,
            show_network: true,
            network_interface: None,
            cpu_warning_threshold: 60.0,
            cpu_critical_threshold: 85.0,
            ram_warning_threshold: 60.0,
            ram_critical_threshold: 85.0,
            gpu_warning_threshold: 60.0,
            gpu_critical_threshold: 85.0,
        }
    }
}

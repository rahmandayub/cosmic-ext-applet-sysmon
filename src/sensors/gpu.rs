// SPDX-License-Identifier: MPL-2.0

//! GPU sensor with vendor-agnostic discovery.
//!
//! Three backends are supported:
//!
//! * **NVIDIA** — discovered via the NVIDIA Management Library (NVML). The
//!   handle is opened at startup via `nvml-wrapper`'s runtime loader, which
//!   means the applet builds and runs on systems that have no NVIDIA
//!   hardware (or driver) at all.
//! * **AMD** — discovered by scanning `/sys/class/drm/card*/device/uevent`
//!   for `DRIVER=amdgpu`. Utilization, VRAM and temperature are read from
//!   `amdgpu`'s sysfs interface.
//! * **Intel** — discovered by scanning the same directory for
//!   `DRIVER=i915` or `DRIVER=xe`. Intel GPUs do not expose a reliable
//!   utilization counter from sysfs, so only temperature is reported.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use nvml_wrapper::error::NvmlError;
use nvml_wrapper::Nvml;

/// One detected GPU.
struct Gpu {
    /// Human-readable name; currently informational only.
    #[allow(dead_code)]
    name: String,
    backend: GpuBackend,
}

/// A single panel line, averaged over every detected GPU.
pub struct Gpus {
    gpus: Vec<Gpu>,
}

/// Per-backend implementation.
enum GpuBackend {
    Nvidia {
        index: u32,
    },
    Amd {
        device_dir: PathBuf,
    },
    Intel {
        device_dir: PathBuf,
    },
}

/// Lazily-initialized NVML handle. `Err` is cached forever; on non-NVIDIA
/// systems `Nvml::init()` returns an error and we simply skip NVIDIA
/// detection.
static NVML: OnceLock<Result<Nvml, NvmlError>> = OnceLock::new();

fn nvml() -> Option<&'static Nvml> {
    NVML.get_or_init(Nvml::init).as_ref().ok()
}

impl Gpus {
    /// Probe `/sys/class/drm` (and NVML) for available GPUs.
    pub fn discover() -> Self {
        let mut gpus = Vec::new();

        if let Some(nvml) = nvml() {
            let count = nvml.device_count().unwrap_or(0);
            for idx in 0..count {
                let name = nvml
                    .device_by_index(idx)
                    .ok()
                    .and_then(|d| d.name().ok())
                    .unwrap_or_else(|| format!("NVIDIA GPU {idx}"));
                gpus.push(Gpu {
                    name,
                    backend: GpuBackend::Nvidia { index: idx },
                });
            }
        }

        if let Ok(entries) = fs::read_dir("/sys/class/drm") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if !name.starts_with("card") || name.contains('-') {
                    // Skip `card0-DP-1`, `renderD128`, etc.
                    continue;
                }
                let device_dir = entry.path().join("device");
                if !device_dir.is_dir() {
                    continue;
                }
                let uevent = match fs::read_to_string(device_dir.join("uevent")) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let driver = uevent
                    .lines()
                    .find_map(|l| l.strip_prefix("DRIVER="))
                    .unwrap_or("");
                let pci_slot = uevent
                    .lines()
                    .find_map(|l| l.strip_prefix("PCI_SLOT_NAME="))
                    .unwrap_or("");

                match driver {
                    "amdgpu" => {
                        // Skip if NVML already reported this PCI device — that
                        // happens on hybrid laptops that route the dGPU
                        // through both drivers.
                        if nvml().is_some() && !pci_slot.is_empty() {
                            if let Some(nvml) = nvml() {
                                let count = nvml.device_count().unwrap_or(0);
                                let mut found = false;
                                for idx in 0..count {
                                    if let Ok(dev) = nvml.device_by_index(idx) {
                                        if let Ok(info) = dev.pci_info() {
                                            if info.bus_id == pci_slot {
                                                found = true;
                                                break;
                                            }
                                        }
                                    }
                                }
                                if found {
                                    continue;
                                }
                            }
                        }
                        gpus.push(Gpu {
                            name: read_name(&device_dir).unwrap_or_else(|| "AMD GPU".into()),
                            backend: GpuBackend::Amd { device_dir },
                        });
                    }
                    "i915" | "xe" => {
                        gpus.push(Gpu {
                            name: read_name(&device_dir)
                                .unwrap_or_else(|| "Intel GPU".into()),
                            backend: GpuBackend::Intel { device_dir },
                        });
                    }
                    _ => {}
                }
            }
        }

        Self { gpus }
    }

    /// Refresh every GPU. Returns
    /// `(utilization_percent, vram_used_bytes, vram_total_bytes, temp_celsius, any_gpu_present)`.
    pub fn refresh(&mut self) -> (Option<f32>, Option<u64>, Option<u64>, Option<f32>, bool) {
        if self.gpus.is_empty() {
            return (None, None, None, None, false);
        }

        let mut util_samples: Vec<f32> = Vec::new();
        let mut vram_used: u64 = 0;
        let mut vram_total: u64 = 0;
        let mut temp_samples: Vec<f32> = Vec::new();
        let mut vram_reported = false;

        for gpu in &self.gpus {
            match &gpu.backend {
                GpuBackend::Nvidia { index } => {
                    if let Some(nvml) = nvml() {
                        if let Ok(device) = nvml.device_by_index(*index) {
                            if let Ok(u) = device.utilization_rates() {
                                util_samples.push(u.gpu as f32);
                            }
                            if let Ok(mem) = device.memory_info() {
                                vram_used = vram_used.saturating_add(mem.used);
                                vram_total = vram_total.saturating_add(mem.total);
                                vram_reported = true;
                            }
                            if let Ok(t) = device.temperature(
                                nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu,
                            ) {
                                temp_samples.push(t as f32);
                            }
                        }
                    }
                }
                GpuBackend::Amd { device_dir } => {
                    if let Some(p) = read_u64(&device_dir.join("gpu_busy_percent")) {
                        util_samples.push(p as f32);
                    }
                    let used = read_u64(&device_dir.join("mem_info_vram_used"));
                    let total = read_u64(&device_dir.join("mem_info_vram_total"));
                    if let (Some(u), Some(t)) = (used, total) {
                        vram_used = vram_used.saturating_add(u);
                        vram_total = vram_total.saturating_add(t);
                        vram_reported = true;
                    }
                    if let Some(c) = read_hwmon_temp_milli(device_dir) {
                        temp_samples.push(c as f32 / 1000.0);
                    }
                }
                GpuBackend::Intel { device_dir } => {
                    if let Some(c) = read_hwmon_temp_milli(device_dir) {
                        temp_samples.push(c as f32 / 1000.0);
                    }
                }
            }
        }

        let util = if util_samples.is_empty() {
            None
        } else {
            Some(util_samples.iter().copied().sum::<f32>() / util_samples.len() as f32)
        };
        let temp = if temp_samples.is_empty() {
            None
        } else {
            // Report the hottest GPU, which is the most useful single number
            // for thermal monitoring.
            temp_samples
                .into_iter()
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        };
        let (vram_used_opt, vram_total_opt) = if vram_reported {
            (Some(vram_used), Some(vram_total))
        } else {
            (None, None)
        };

        (util, vram_used_opt, vram_total_opt, temp, true)
    }
}

/// Read `name` from `device_dir`, returning `None` if it cannot be read.
fn read_name(device_dir: &Path) -> Option<String> {
    let raw = fs::read_to_string(device_dir.join("product_name"))
        .or_else(|_| fs::read_to_string(device_dir.join("vendor"))) // last-ditch fallback
        .ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Read a file, trim it, and parse it as a `u64`. Returns `None` on any
/// error.
fn read_u64(path: &Path) -> Option<u64> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
}

/// Walk `device_dir/hwmon/hwmon*/temp*_input`, prefer the entry whose
/// sibling `temp*_label` is `edge`, and fall back to `temp1_input`.
/// Returned value is in millidegrees Celsius.
fn read_hwmon_temp_milli(device_dir: &Path) -> Option<u32> {
    let hwmon_root = device_dir.join("hwmon");
    let entries = fs::read_dir(&hwmon_root).ok()?;

    let mut fallback: Option<u32> = None;

    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        for temp_entry in fs::read_dir(&dir).ok()?.flatten() {
            let p = temp_entry.path();
            let Some(fname) = p.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if !fname.starts_with("temp") || !fname.ends_with("_input") {
                continue;
            }
            let label_path = p.with_file_name(fname.replace("_input", "_label"));
            let label = fs::read_to_string(&label_path)
                .ok()
                .map(|s| s.trim().to_string());
            let raw = fs::read_to_string(&p).ok()?;
            let value: u32 = raw.trim().parse().ok()?;
            if label.as_deref() == Some("edge") {
                return Some(value);
            }
            if fname == "temp1_input" && fallback.is_none() {
                fallback = Some(value);
            }
        }
    }

    fallback
}

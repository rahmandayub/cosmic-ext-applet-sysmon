// SPDX-License-Identifier: MPL-2.0

//! Small helpers used to format sensor values for display in the panel and the
//! popup.

/// Format a byte count with binary unit suffixes (B / KiB / MiB / GiB / TiB),
/// showing at most one decimal of precision once the value is at least 1 KiB.
pub fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    const TIB: f64 = GIB * 1024.0;

    let b = bytes as f64;
    if b >= TIB {
        format!("{:.1} TiB", b / TIB)
    } else if b >= GIB {
        format!("{:.1} GiB", b / GIB)
    } else if b >= MIB {
        format!("{:.1} MiB", b / MIB)
    } else if b >= KIB {
        format!("{:.1} KiB", b / KIB)
    } else {
        format!("{} B", bytes)
    }
}

/// Format a throughput (bytes per second) using decimal unit suffixes
/// (B/s, KB/s, MB/s, GB/s), with at most one decimal of precision.
pub fn format_bytes_per_sec(bps: f64) -> String {
    const KB: f64 = 1000.0;
    const MB: f64 = KB * 1000.0;
    const GB: f64 = MB * 1000.0;

    if !bps.is_finite() || bps < 0.0 {
        return "0 B/s".to_string();
    }
    if bps >= GB {
        format!("{:.1} GB/s", bps / GB)
    } else if bps >= MB {
        format!("{:.1} MB/s", bps / MB)
    } else if bps >= KB {
        format!("{:.1} KB/s", bps / KB)
    } else {
        format!("{:.0} B/s", bps)
    }
}

/// Format a 0.0–100.0 percentage as a whole number with a trailing `%`.
pub fn format_percent(value: f32) -> String {
    if !value.is_finite() {
        return "0%".to_string();
    }
    let clamped = value.clamp(0.0, 100.0);
    format!("{}%", clamped.round() as u32)
}

/// Format a temperature (in °C) with the degree-Celsius suffix.
pub fn format_temp(value: f32) -> String {
    if !value.is_finite() {
        return "--°C".to_string();
    }
    format!("{}°C", value.round() as i32)
}

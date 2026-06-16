// SPDX-License-Identifier: MPL-2.0

use cosmic::iced::Color;

// Static title colors
pub const CPU_COLOR: Color = Color {
    r: 0.29,
    g: 0.56,
    b: 0.85,
    a: 1.0,
};
pub const RAM_COLOR: Color = Color {
    r: 0.31,
    g: 0.78,
    b: 0.47,
    a: 1.0,
};
pub const GPU_COLOR: Color = Color {
    r: 0.61,
    g: 0.35,
    b: 0.71,
    a: 1.0,
};
pub const NET_COLOR: Color = Color {
    r: 0.90,
    g: 0.49,
    b: 0.13,
    a: 1.0,
};

// Network arrow colors
pub const NET_RX_COLOR: Color = RAM_COLOR;
pub const NET_TX_COLOR: Color = CPU_COLOR;

// Threshold colors
pub const WARNING_COLOR: Color = Color {
    r: 0.95,
    g: 0.61,
    b: 0.07,
    a: 1.0,
};
pub const CRITICAL_COLOR: Color = Color {
    r: 0.91,
    g: 0.30,
    b: 0.24,
    a: 1.0,
};

/// Returns the appropriate color for a usage percentage based on thresholds.
/// - Below warning: `None` (use theme default)
/// - Warning to critical: `Some(WARNING_COLOR)`
/// - Above critical: `Some(CRITICAL_COLOR)`
pub fn threshold_color(percent: f32, warning: f32, critical: f32) -> Option<Color> {
    if percent > critical {
        Some(CRITICAL_COLOR)
    } else if percent > warning {
        Some(WARNING_COLOR)
    } else {
        None
    }
}

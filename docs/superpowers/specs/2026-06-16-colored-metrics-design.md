# Design: Colored Metrics Panel

## Overview

Add color differentiation to COSMIC Sysmon panel metrics:
- Static colors for metric titles (CPU, RAM, GPU, NET)
- Dynamic threshold-based colors for usage values (CPU, RAM, GPU)
- Colored download/upload arrows for network

## Requirements

1. Title colors: Static per metric (blue, green, purple, orange)
2. Usage colors: Dynamic based on configurable thresholds (theme → yellow → red)
3. Network arrows: Static colors (green ↓, blue ↑)
4. Thresholds: User-configurable via popup settings

## Color Specifications

### Static Title Colors

| Metric | Hex | Usage |
|--------|-----|-------|
| CPU | `#4A90D9` | Title text |
| RAM | `#50C878` | Title text |
| GPU | `#9B59B6` | Title text |
| NET | `#E67E22` | Title text |

### Dynamic Threshold Colors

Applied to usage values (CPU%, RAM%, GPU%):

| Range | Hex | Usage |
|-------|-----|-------|
| 0% - warning_threshold | Theme default | Normal |
| warning_threshold - critical_threshold | `#F39C12` | Warning |
| > critical_threshold | `#E74C3C` | Critical |

### Network Arrow Colors

| Element | Hex |
|---------|-----|
| ↓ (Download) | `#50C878` |
| ↑ (Upload) | `#4A90D9` |

## Implementation

### 1. Config Changes (`src/config.rs`)

Add threshold fields:

```rust
pub cpu_warning_threshold: f32,    // default 60.0
pub cpu_critical_threshold: f32,   // default 85.0
pub ram_warning_threshold: f32,    // default 60.0
pub ram_critical_threshold: f32,   // default 85.0
pub gpu_warning_threshold: f32,    // default 60.0
pub gpu_critical_threshold: f32,   // default 85.0
```

### 2. Color Module (`src/color.rs`)

New module defining:
- Static title colors as `cosmic::Color`
- Threshold color function: `fn threshold_color(percent: f32, warning: f32, critical: f32) -> cosmic::Color`
- Network arrow colors

### 3. Panel Rendering (`src/app.rs`)

Replace `build_panel_line()` with `build_panel_rich_text()` that returns `Element` with:
- `widget::rich_text` containing spans with individual colors
- Each metric title + value as styled spans
- Network with colored arrows

### 4. Popup Settings (`src/app.rs`)

Add threshold sliders to popup:
- 2 sliders per metric (warning, critical)
- Range: 0-100%
- Labels show current value

## File Changes

| File | Change |
|------|--------|
| `src/config.rs` | Add 6 threshold fields |
| `src/color.rs` | New module - color constants + functions |
| `src/app.rs` | Refactor panel rendering + add threshold UI |
| `src/main.rs` | Add `mod color` |

## UI Behavior

- Below warning threshold: theme default color
- Warning to critical: yellow
- Above critical: red
- Panel text uses `rich_text` for colored spans
- Popup shows threshold sliders with live preview

## Testing

- Visual: Verify colors render correctly in panel
- Config: Verify thresholds persist across restarts
- Edge cases: 0%, 100%, exact threshold boundaries

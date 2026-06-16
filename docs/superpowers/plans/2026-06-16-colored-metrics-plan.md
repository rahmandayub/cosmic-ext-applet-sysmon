# Colored Metrics Panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add color differentiation to panel metrics with static title colors and dynamic threshold-based usage colors.

**Architecture:** Create a new color module for constants and threshold logic, update config for threshold settings, refactor panel rendering to use rich_text for colored spans, and add threshold sliders to popup settings.

**Tech Stack:** Rust, libcosmic (iced), cosmic-config

---

## File Structure

| File | Responsibility |
|------|----------------|
| `src/color.rs` | Color constants, threshold color function |
| `src/config.rs` | Add threshold configuration fields |
| `src/app.rs` | Refactor panel rendering, add threshold UI |
| `src/main.rs` | Add `mod color` declaration |

---

### Task 1: Create Color Module

**Files:**
- Create: `src/color.rs`

- [ ] **Step 1: Create color module with constants and threshold function**

```rust
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
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/color.rs
git commit -m "feat: add color module with title and threshold colors"
```

---

### Task 2: Update Config with Thresholds

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Add threshold fields to Config struct**

Add these fields to the `Config` struct after `network_interface`:

```rust
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
```

- [ ] **Step 2: Add default values in Default impl**

Add to the `Default` implementation:

```rust
cpu_warning_threshold: 60.0,
cpu_critical_threshold: 85.0,
ram_warning_threshold: 60.0,
ram_critical_threshold: 85.0,
gpu_warning_threshold: 60.0,
gpu_critical_threshold: 85.0,
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/config.rs
git commit -m "feat: add configurable threshold settings"
```

---

### Task 3: Add Threshold Message Variants

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Add new Message variants for threshold changes**

Add to the `Message` enum:

```rust
/// The user changed a threshold slider.
SetThreshold(ThresholdMetric, ThresholdLevel, f32),
```

- [ ] **Step 2: Add ThresholdMetric and ThresholdLevel enums**

Add after the `Metric` enum:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThresholdMetric {
    Cpu,
    Memory,
    Gpu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThresholdLevel {
    Warning,
    Critical,
}
```

- [ ] **Step 3: Add threshold update handler in update method**

Add match arm in `update` method:

```rust
Message::SetThreshold(metric, level, value) => {
    let mut new = self.config.clone();
    let v = value.clamp(0.0, 100.0);
    match (metric, level) {
        (ThresholdMetric::Cpu, ThresholdLevel::Warning) => new.cpu_warning_threshold = v,
        (ThresholdMetric::Cpu, ThresholdLevel::Critical) => new.cpu_critical_threshold = v,
        (ThresholdMetric::Memory, ThresholdLevel::Warning) => new.ram_warning_threshold = v,
        (ThresholdMetric::Memory, ThresholdLevel::Critical) => new.ram_critical_threshold = v,
        (ThresholdMetric::Gpu, ThresholdLevel::Warning) => new.gpu_warning_threshold = v,
        (ThresholdMetric::Gpu, ThresholdLevel::Critical) => new.gpu_critical_threshold = v,
    }
    if let Ok(context) = cosmic_config::Config::new(Self::APP_ID, Config::VERSION) {
        let _ = new.write_entry(&context);
    }
    self.config = new;
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat: add threshold message variants and handler"
```

---

### Task 4: Refactor Panel to Rich Text

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Replace build_panel_line with build_panel_rich_text**

Replace the `build_panel_line` function with:

```rust
use cosmic::widget::text::Span;
use crate::color;

fn build_panel_rich_text(s: &SensorsSnapshot, cfg: &Config) -> Option<Element<'static, Message>> {
    let mut spans: Vec<Span<'static, Message>> = Vec::new();

    if cfg.show_cpu {
        let cpu_color = color::CPU_COLOR;
        spans.push(Span::styled("CPU", cpu_color));
        
        let cpu_pct = fmt::format_percent(s.cpu_percent);
        let value_color = color::threshold_color(
            s.cpu_percent,
            cfg.cpu_warning_threshold,
            cfg.cpu_critical_threshold,
        );
        if let Some(c) = value_color {
            spans.push(Span::styled(format!(" {cpu_pct}"), c));
        } else {
            spans.push(Span::text(format!(" {cpu_pct}")));
        }
        spans.push(Span::text(" | "));
    }

    if cfg.show_memory {
        let ram_color = color::RAM_COLOR;
        spans.push(Span::styled("RAM", ram_color));
        
        let used = fmt::format_bytes(s.memory_used);
        let total = fmt::format_bytes(s.memory_total);
        let ram_pct = (s.memory_used as f32 / s.memory_total as f32) * 100.0;
        let value_color = color::threshold_color(
            ram_pct,
            cfg.ram_warning_threshold,
            cfg.ram_critical_threshold,
        );
        if let Some(c) = value_color {
            spans.push(Span::styled(format!(" {used}/{total}"), c));
        } else {
            spans.push(Span::text(format!(" {used}/{total}")));
        }
        spans.push(Span::text(" | "));
    }

    if cfg.show_gpu && s.gpu_present {
        let gpu_color = color::GPU_COLOR;
        spans.push(Span::styled("GPU", gpu_color));
        
        if let Some(p) = s.gpu_percent {
            let value_color = color::threshold_color(
                p,
                cfg.gpu_warning_threshold,
                cfg.gpu_critical_threshold,
            );
            if let Some(c) = value_color {
                spans.push(Span::styled(format!(" {p:.0}%"), c));
            } else {
                spans.push(Span::text(format!(" {p:.0}%")));
            }
        }
        if let (Some(used), Some(total)) = (s.gpu_vram_used, s.gpu_vram_total) {
            let vram_str = format!(" | {}/{}", fmt::format_bytes(used), fmt::format_bytes(total));
            spans.push(Span::text(vram_str));
        }
        if let Some(t) = s.gpu_temp_c {
            spans.push(Span::text(format!(" {}", fmt::format_temp(t))));
        }
        spans.push(Span::text(" | "));
    }

    if cfg.show_network {
        let net_color = color::NET_COLOR;
        spans.push(Span::styled("NET", net_color));
        
        let rx = fmt::format_bytes_per_sec(s.net_rx_bps);
        let tx = fmt::format_bytes_per_sec(s.net_tx_bps);
        let iface = s.net_interface.as_deref().unwrap_or("?");
        
        spans.push(Span::styled(format!(" {iface}"), color::NET_RX_COLOR));
        spans.push(Span::styled(format!(" ↓{rx}"), color::NET_RX_COLOR));
        spans.push(Span::styled(format!(" ↑{tx}"), color::NET_TX_COLOR));
    }

    if spans.is_empty() {
        return None;
    }

    // Remove trailing " | "
    if let Some(last) = spans.last() {
        if last.text == " | " {
            spans.pop();
        }
    }

    Some(widget::rich_text(spans).into())
}
```

- [ ] **Step 2: Update view method to use rich text**

Replace the `view` method body:

```rust
fn view(&self) -> Element<'_, Self::Message> {
    let content = build_panel_rich_text(&self.snapshot, &self.config)
        .unwrap_or_else(|| widget::text("Sysmon (no metrics)").into());

    let (_, panel_h) = self.core.applet.suggested_window_size();
    let applet_icon = widget::container(content)
        .height(cosmic::iced::Length::Fixed(panel_h.get() as f32))
        .padding([0.0, 12.0])
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);

    widget::button::custom(self.core.applet.autosize_window(applet_icon))
        .on_press(Message::TogglePopup)
        .class(cosmic::theme::Button::AppletIcon)
        .into()
}
```

- [ ] **Step 3: Remove old build_panel_line function**

Delete the `build_panel_line` function (lines 358-397).

- [ ] **Step 4: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat: refactor panel to use rich text with colors"
```

---

### Task 5: Add Threshold Sliders to Popup

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Add threshold slider section in view_window**

Add after the refresh interval slider in `view_window`:

```rust
// Threshold settings section
let threshold_header = widget::text(fl!("threshold-settings"))
    .font(cosmic::theme::Font::Semibold);
list = list.add(threshold_header);

// CPU thresholds
let cpu_warning_slider = widget::slider(
    0.0..=100.0,
    self.config.cpu_warning_threshold,
    move |v| Message::SetThreshold(ThresholdMetric::Cpu, ThresholdLevel::Warning, v),
);
let cpu_warning_label = format!("CPU {}%: {:.0}%", fl!("warning"), self.config.cpu_warning_threshold);
list = list.add(widget::settings::item(cpu_warning_label, cpu_warning_slider));

let cpu_critical_slider = widget::slider(
    0.0..=100.0,
    self.config.cpu_critical_threshold,
    move |v| Message::SetThreshold(ThresholdMetric::Cpu, ThresholdLevel::Critical, v),
);
let cpu_critical_label = format!("CPU {}%: {:.0}%", fl!("critical"), self.config.cpu_critical_threshold);
list = list.add(widget::settings::item(cpu_critical_label, cpu_critical_slider));

// RAM thresholds
let ram_warning_slider = widget::slider(
    0.0..=100.0,
    self.config.ram_warning_threshold,
    move |v| Message::SetThreshold(ThresholdMetric::Memory, ThresholdLevel::Warning, v),
);
let ram_warning_label = format!("RAM {}%: {:.0}%", fl!("warning"), self.config.ram_warning_threshold);
list = list.add(widget::settings::item(ram_warning_label, ram_warning_slider));

let ram_critical_slider = widget::slider(
    0.0..=100.0,
    self.config.ram_critical_threshold,
    move |v| Message::SetThreshold(ThresholdMetric::Memory, ThresholdLevel::Critical, v),
);
let ram_critical_label = format!("RAM {}%: {:.0}%", fl!("critical"), self.config.ram_critical_threshold);
list = list.add(widget::settings::item(ram_critical_label, ram_critical_slider));

// GPU thresholds (only if GPU is present)
if self.snapshot.gpu_present {
    let gpu_warning_slider = widget::slider(
        0.0..=100.0,
        self.config.gpu_warning_threshold,
        move |v| Message::SetThreshold(ThresholdMetric::Gpu, ThresholdLevel::Warning, v),
    );
    let gpu_warning_label = format!("GPU {}%: {:.0}%", fl!("warning"), self.config.gpu_warning_threshold);
    list = list.add(widget::settings::item(gpu_warning_label, gpu_warning_slider));

    let gpu_critical_slider = widget::slider(
        0.0..=100.0,
        self.config.gpu_critical_threshold,
        move |v| Message::SetThreshold(ThresholdMetric::Gpu, ThresholdLevel::Critical, v),
    );
    let gpu_critical_label = format!("GPU {}%: {:.0}%", fl!("critical"), self.config.gpu_critical_threshold);
    list = list.add(widget::settings::item(gpu_critical_label, gpu_critical_slider));
}
```

- [ ] **Step 2: Add i18n strings**

Create/update `i18n/en/main.ftl`:

```ftl
threshold-settings = Threshold Settings
warning = Warning
critical = Critical
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/app.rs i18n/
git commit -m "feat: add threshold sliders to popup settings"
```

---

### Task 6: Final Verification

**Files:**
- None (verification only)

- [ ] **Step 1: Run full build**

Run: `cargo build`
Expected: PASS with no warnings

- [ ] **Step 2: Test visually**

Run the applet and verify:
- Panel shows colored metric titles
- Usage values change color at thresholds
- Network arrows are colored (green ↓, blue ↑)
- Threshold sliders work in popup

- [ ] **Step 3: Final commit**

```bash
git add -A
git commit -m "feat: complete colored metrics panel implementation"
```

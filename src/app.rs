// SPDX-License-Identifier: MPL-2.0

use std::time::{Duration, Instant};

use crate::config::{
    Config, DEFAULT_REFRESH_INTERVAL_MS, MAX_REFRESH_INTERVAL_MS, MIN_REFRESH_INTERVAL_MS,
};
use crate::fl;
use crate::format as fmt;
use crate::sensors::{Sensors, SensorsSnapshot};

use cosmic::cosmic_config::{self, CosmicConfigEntry};

use cosmic::iced::platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup};
use cosmic::iced::{time, window::Id, Alignment, Limits, Subscription};
use cosmic::prelude::*;
use cosmic::widget;
use cosmic::iced::widget::text::Span;
use crate::color;

/// The application model stores app-specific state used to describe its interface and
/// drive its logic.
pub struct AppModel {
    /// Application state which is managed by the COSMIC runtime.
    core: cosmic::Core,
    /// The popup id.
    popup: Option<Id>,
    /// Configuration data that persists between application runs.
    config: Config,
    /// Sensor handles.
    sensors: Sensors,
    /// Last sensor reading. Updated on every `Tick`.
    snapshot: SensorsSnapshot,
    /// Time of the previous tick. Used to compute elapsed time for rate
    /// calculations.
    last_tick: Instant,
}

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    /// A periodic tick that triggers a sensor refresh.
    Tick,
    /// The user toggled one of the metric checkboxes in the popup.
    ToggleMetric(Metric, bool),
    /// The user picked a network interface (or `None` for auto).
    SelectNetworkInterface(Option<String>),
    /// The user changed the refresh interval via the slider.
    SetRefreshInterval(f32),
    /// The user changed a threshold slider.
    SetThreshold(ThresholdMetric, ThresholdLevel, f32),
    UpdateConfig(Config),
}

/// Which metric the user can toggle in the popup checklist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    Cpu,
    Memory,
    Gpu,
    Network,
}

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

/// Create a COSMIC application from the app model
impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = "com.github.rahmandayub.Sysmon";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    fn init(
        core: cosmic::Core,
        _flags: Self::Flags,
    ) -> (Self, Task<cosmic::Action<Self::Message>>) {
        let config = cosmic_config::Config::new(Self::APP_ID, Config::VERSION)
            .map(|context| match Config::get_entry(&context) {
                Ok(config) => config,
                Err((_errors, config)) => config,
            })
            .unwrap_or_default();

        let mut sensors = Sensors::new();
        // Prime the first refresh so the panel shows values immediately
        // (CPU usage needs two reads; this also warms up network counters).
        let snapshot = sensors.refresh(
            Duration::from_millis(config.refresh_interval_ms.max(MIN_REFRESH_INTERVAL_MS)),
            config.network_interface.as_deref(),
        );

        let app = AppModel {
            core,
            config,
            sensors,
            snapshot,
            last_tick: Instant::now(),
            popup: None,
        };

        (app, Task::none())
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    /// Panel view: a single line with the requested metrics, clickable to
    /// open the popup.
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

    /// Popup view: a checklist of metrics plus a NIC dropdown.
    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        let mut list = widget::list_column();

        list = list
            .add(settings_item_toggle(
                fl!("show-cpu"),
                self.config.show_cpu,
                |b| Message::ToggleMetric(Metric::Cpu, b),
            ))
            .add(settings_item_toggle(
                fl!("show-memory"),
                self.config.show_memory,
                |b| Message::ToggleMetric(Metric::Memory, b),
            ));

        if self.snapshot.gpu_present {
            list = list.add(settings_item_toggle(
                fl!("show-gpu"),
                self.config.show_gpu,
                |b| Message::ToggleMetric(Metric::Gpu, b),
            ));
        }

        list = list.add(settings_item_toggle(
            fl!("show-network"),
            self.config.show_network,
            |b| Message::ToggleMetric(Metric::Network, b),
        ));

        // Network interface selector (only shown when network is enabled).
        if self.config.show_network && !self.snapshot.net_available.is_empty() {
            let auto_label = fl!("auto");
            let nic_options: Vec<String> = std::iter::once(auto_label.clone())
                .chain(self.snapshot.net_available.iter().cloned())
                .collect();

            let current = self
                .config
                .network_interface
                .clone()
                .unwrap_or_else(|| auto_label.clone());
            let current_idx = nic_options
                .iter()
                .position(|n| n == &current)
                .unwrap_or(0);
            let auto_for_cmp = auto_label.clone();
            let dropdown = {
                let opts = nic_options.clone();
                widget::dropdown(nic_options, Some(current_idx), move |idx| {
                    let chosen = opts[idx].clone();
                    if chosen == auto_for_cmp {
                        Message::SelectNetworkInterface(None)
                    } else {
                        Message::SelectNetworkInterface(Some(chosen))
                    }
                })
            };

            list = list.add(widget::settings::item(
                fl!("network-interface"),
                dropdown,
            ));
        }

        // Refresh interval slider. The slider requires `T: Into<f64>` to
        // become an `Element`, so we use `f32` here and convert back to
        // `u64` when persisting the value.
        let slider = widget::slider(
            (MIN_REFRESH_INTERVAL_MS as f32)..=(MAX_REFRESH_INTERVAL_MS as f32),
            self.config.refresh_interval_ms as f32,
            Message::SetRefreshInterval,
        );
        let interval_label = format!("{} ms", self.config.refresh_interval_ms);
        list = list.add(widget::settings::item(interval_label, slider));

        // Threshold settings section
        let threshold_header = widget::text(fl!("threshold-settings"));
        list = list.add(threshold_header);

        // CPU thresholds
        let cpu_warning_slider = widget::slider(
            0.0..=100.0,
            self.config.cpu_warning_threshold,
            move |v| Message::SetThreshold(ThresholdMetric::Cpu, ThresholdLevel::Warning, v),
        );
        let cpu_warning_label = format!("CPU {}: {:.0}%", fl!("warning"), self.config.cpu_warning_threshold);
        list = list.add(widget::settings::item(cpu_warning_label, cpu_warning_slider));

        let cpu_critical_slider = widget::slider(
            0.0..=100.0,
            self.config.cpu_critical_threshold,
            move |v| Message::SetThreshold(ThresholdMetric::Cpu, ThresholdLevel::Critical, v),
        );
        let cpu_critical_label = format!("CPU {}: {:.0}%", fl!("critical"), self.config.cpu_critical_threshold);
        list = list.add(widget::settings::item(cpu_critical_label, cpu_critical_slider));

        // RAM thresholds
        let ram_warning_slider = widget::slider(
            0.0..=100.0,
            self.config.ram_warning_threshold,
            move |v| Message::SetThreshold(ThresholdMetric::Memory, ThresholdLevel::Warning, v),
        );
        let ram_warning_label = format!("RAM {}: {:.0}%", fl!("warning"), self.config.ram_warning_threshold);
        list = list.add(widget::settings::item(ram_warning_label, ram_warning_slider));

        let ram_critical_slider = widget::slider(
            0.0..=100.0,
            self.config.ram_critical_threshold,
            move |v| Message::SetThreshold(ThresholdMetric::Memory, ThresholdLevel::Critical, v),
        );
        let ram_critical_label = format!("RAM {}: {:.0}%", fl!("critical"), self.config.ram_critical_threshold);
        list = list.add(widget::settings::item(ram_critical_label, ram_critical_slider));

        // GPU thresholds (only if GPU is present)
        if self.snapshot.gpu_present {
            let gpu_warning_slider = widget::slider(
                0.0..=100.0,
                self.config.gpu_warning_threshold,
                move |v| Message::SetThreshold(ThresholdMetric::Gpu, ThresholdLevel::Warning, v),
            );
            let gpu_warning_label = format!("GPU {}: {:.0}%", fl!("warning"), self.config.gpu_warning_threshold);
            list = list.add(widget::settings::item(gpu_warning_label, gpu_warning_slider));

            let gpu_critical_slider = widget::slider(
                0.0..=100.0,
                self.config.gpu_critical_threshold,
                move |v| Message::SetThreshold(ThresholdMetric::Gpu, ThresholdLevel::Critical, v),
            );
            let gpu_critical_label = format!("GPU {}: {:.0}%", fl!("critical"), self.config.gpu_critical_threshold);
            list = list.add(widget::settings::item(gpu_critical_label, gpu_critical_slider));
        }

        let content = widget::column([list.into()])
            .padding(12);

        self.core.applet.popup_container(content).into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let interval = self
            .config
            .refresh_interval_ms
            .clamp(MIN_REFRESH_INTERVAL_MS, MAX_REFRESH_INTERVAL_MS)
            .max(MIN_REFRESH_INTERVAL_MS);

        Subscription::batch(vec![
            time::every(Duration::from_millis(interval)).map(|_| Message::Tick),
            self.core()
                .watch_config::<Config>(Self::APP_ID)
                .map(|update| Message::UpdateConfig(update.config)),
        ])
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::Tick => {
                let now = Instant::now();
                let elapsed = now.duration_since(self.last_tick);
                self.last_tick = now;
                let configured = Duration::from_millis(
                    self.config
                        .refresh_interval_ms
                        .clamp(MIN_REFRESH_INTERVAL_MS, MAX_REFRESH_INTERVAL_MS),
                );
                // Snap to the configured interval so rate numbers are
                // consistent regardless of timer jitter.
                let delta = if elapsed < Duration::from_millis(50) {
                    configured
                } else {
                    elapsed
                };
                self.snapshot = self
                    .sensors
                    .refresh(delta, self.config.network_interface.as_deref());
            }
            Message::UpdateConfig(config) => {
                self.config = config;
            }
            Message::ToggleMetric(metric, on) => {
                let mut new = self.config.clone();
                match metric {
                    Metric::Cpu => new.show_cpu = on,
                    Metric::Memory => new.show_memory = on,
                    Metric::Gpu => new.show_gpu = on,
                    Metric::Network => new.show_network = on,
                }
                if let Ok(context) = cosmic_config::Config::new(Self::APP_ID, Config::VERSION) {
                    let _ = new.write_entry(&context);
                }
                self.config = new;
            }
            Message::SelectNetworkInterface(choice) => {
                let mut new = self.config.clone();
                new.network_interface = choice;
                if let Ok(context) = cosmic_config::Config::new(Self::APP_ID, Config::VERSION) {
                    let _ = new.write_entry(&context);
                }
                self.config = new;
            }
            Message::SetRefreshInterval(value) => {
                let mut new = self.config.clone();
                let v = value
                    .round()
                    .clamp(
                        MIN_REFRESH_INTERVAL_MS as f32,
                        MAX_REFRESH_INTERVAL_MS as f32,
                    ) as u64;
                new.refresh_interval_ms = v;
                if let Ok(context) = cosmic_config::Config::new(Self::APP_ID, Config::VERSION) {
                    let _ = new.write_entry(&context);
                }
                self.config = new;
            }
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
            Message::TogglePopup => {
                return if let Some(p) = self.popup.take() {
                    destroy_popup(p)
                } else {
                    let new_id = Id::unique();
                    self.popup.replace(new_id);
                    let mut popup_settings = self.core.applet.get_popup_settings(
                        self.core.main_window_id().unwrap(),
                        new_id,
                        None,
                        None,
                        None,
                    );
                    popup_settings.positioner.size_limits = Limits::NONE
                        .max_width(372.0)
                        .min_width(300.0)
                        .min_height(200.0)
                        .max_height(1080.0);
                    get_popup(popup_settings)
                };
            }
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                }
            }
        }
        Task::none()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

impl Default for AppModel {
    fn default() -> Self {
        let mut sensors = Sensors::new();
        let snapshot = sensors.refresh(
            Duration::from_millis(DEFAULT_REFRESH_INTERVAL_MS),
            None,
        );
        Self {
            core: cosmic::Core::default(),
            popup: None,
            config: Config::default(),
            sensors,
            snapshot,
            last_tick: Instant::now(),
        }
    }
}

/// Build the single-line text shown in the panel.
fn build_panel_rich_text(s: &SensorsSnapshot, cfg: &Config) -> Option<Element<'static, Message>> {
    let mut spans: Vec<Span<'static, Message>> = Vec::new();

    if cfg.show_cpu {
        let cpu_color = color::CPU_COLOR;
        spans.push(Span::new("CPU").color(cpu_color));
        
        let cpu_pct = fmt::format_percent(s.cpu_percent);
        let value_color = color::threshold_color(
            s.cpu_percent,
            cfg.cpu_warning_threshold,
            cfg.cpu_critical_threshold,
        );
        if let Some(c) = value_color {
            spans.push(Span::new(format!(" {cpu_pct}")).color(c));
        } else {
            spans.push(Span::new(format!(" {cpu_pct}")));
        }
        spans.push(Span::new(" | "));
    }

    if cfg.show_memory {
        let ram_color = color::RAM_COLOR;
        spans.push(Span::new("RAM").color(ram_color));
        
        let used = fmt::format_bytes(s.memory_used);
        let total = fmt::format_bytes(s.memory_total);
        let ram_pct = (s.memory_used as f32 / s.memory_total as f32) * 100.0;
        let value_color = color::threshold_color(
            ram_pct,
            cfg.ram_warning_threshold,
            cfg.ram_critical_threshold,
        );
        if let Some(c) = value_color {
            spans.push(Span::new(format!(" {used}/{total}")).color(c));
        } else {
            spans.push(Span::new(format!(" {used}/{total}")));
        }
        spans.push(Span::new(" | "));
    }

    if cfg.show_gpu && s.gpu_present {
        let gpu_color = color::GPU_COLOR;
        spans.push(Span::new("GPU").color(gpu_color));
        
        if let Some(p) = s.gpu_percent {
            let value_color = color::threshold_color(
                p,
                cfg.gpu_warning_threshold,
                cfg.gpu_critical_threshold,
            );
            if let Some(c) = value_color {
                spans.push(Span::new(format!(" {p:.0}%")).color(c));
            } else {
                spans.push(Span::new(format!(" {p:.0}%")));
            }
        }
        if let (Some(used), Some(total)) = (s.gpu_vram_used, s.gpu_vram_total) {
            let vram_str = format!(" | {}/{}", fmt::format_bytes(used), fmt::format_bytes(total));
            spans.push(Span::new(vram_str));
        }
        if let Some(t) = s.gpu_temp_c {
            spans.push(Span::new(format!(" {}", fmt::format_temp(t))));
        }
        spans.push(Span::new(" | "));
    }

    if cfg.show_network {
        let net_color = color::NET_COLOR;
        spans.push(Span::new("NET").color(net_color));
        
        let rx = fmt::format_bytes_per_sec(s.net_rx_bps);
        let tx = fmt::format_bytes_per_sec(s.net_tx_bps);
        let iface = s.net_interface.as_deref().unwrap_or("?");
        
        spans.push(Span::new(format!(" {iface}")));
        spans.push(Span::new(" ↓").color(color::NET_RX_COLOR));
        spans.push(Span::new(rx));
        spans.push(Span::new(" ↑").color(color::NET_TX_COLOR));
        spans.push(Span::new(tx));
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

    Some(cosmic::iced::widget::rich_text(spans).into())
}

/// Helper to build a `settings::item` row that has a toggler on the right.
fn settings_item_toggle<F>(
    label: String,
    active: bool,
    on_toggle: F,
) -> cosmic::Element<'static, Message>
where
    F: Fn(bool) -> Message + Send + Sync + 'static,
{
    widget::settings::item(label, widget::toggler(active).on_toggle(on_toggle)).into()
}

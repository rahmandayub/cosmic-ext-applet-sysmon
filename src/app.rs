// SPDX-License-Identifier: MPL-2.0

use std::time::{Duration, Instant};

use crate::config::{
    Config, DEFAULT_REFRESH_INTERVAL_MS, MAX_REFRESH_INTERVAL_MS, MIN_REFRESH_INTERVAL_MS,
};
use crate::fl;
use crate::format as fmt;
use crate::sensors::{Sensors, SensorsSnapshot};

use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::core::text::Wrapping;
use cosmic::iced::platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup};
use cosmic::iced::{time, window::Id, Alignment, Limits, Subscription};
use cosmic::prelude::*;
use cosmic::widget;

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
        let line = build_panel_line(&self.snapshot, &self.config);
        // `Wrapping::None` keeps the metrics on one line and lets the applet
        // grow horizontally with its content. The whole content is wrapped in
        // `self.core.applet.autosize_window(...)` so the applet window itself
        // resizes to fit the text + icon instead of staying at icon-width
        // (which was the default initial size in the applet's window
        // settings).
        let text = widget::text(line)
            .wrapping(Wrapping::None)
            .shaping(cosmic::iced::core::text::Shaping::Advanced);

        // The container fixes the content to the applet slot height (the
        // panel's `panel_h`) and centers it. Without this, the autosize
        // widget would treat the content's height as 0 and the applet
        // would collapse vertically.
        let (_, panel_h) = self.core.applet.suggested_window_size();
        let applet_icon = widget::container(text)
            .height(cosmic::iced::Length::Fixed(panel_h.get() as f32))
            .padding([0.0, 12.0])
            .align_x(Alignment::Center)
            .align_y(Alignment::Center);

        // `autosize_window` wraps the content in an autosize container that
        // resizes the applet window to fit it. This is what makes the
        // applet width adapt to the text content instead of staying at
        // icon-width.
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
fn build_panel_line(s: &SensorsSnapshot, cfg: &Config) -> String {
    let mut parts: Vec<String> = Vec::new();

    if cfg.show_cpu {
        parts.push(format!("CPU {}", fmt::format_percent(s.cpu_percent)));
    }
    if cfg.show_memory {
        let used = fmt::format_bytes(s.memory_used);
        let total = fmt::format_bytes(s.memory_total);
        parts.push(format!("RAM {used}/{total}"));
    }
    if cfg.show_gpu && s.gpu_present {
        let mut gpu_line = String::from("GPU");
        if let Some(p) = s.gpu_percent {
            gpu_line.push_str(&format!(" {}", fmt::format_percent(p)));
        }
        if let (Some(used), Some(total)) = (s.gpu_vram_used, s.gpu_vram_total) {
            gpu_line.push_str(&format!(" | {}", fmt::format_bytes(used)));
            if total > 0 {
                gpu_line.push_str(&format!("/{}", fmt::format_bytes(total)));
            }
        }
        if let Some(t) = s.gpu_temp_c {
            gpu_line.push_str(&format!(" {}", fmt::format_temp(t)));
        }
        parts.push(gpu_line);
    }
    if cfg.show_network {
        let rx = fmt::format_bytes_per_sec(s.net_rx_bps);
        let tx = fmt::format_bytes_per_sec(s.net_tx_bps);
        let iface = s.net_interface.as_deref().unwrap_or("?");
        parts.push(format!("{iface} ↓{rx} ↑{tx}"));
    }

    if parts.is_empty() {
        "Sysmon (no metrics)".to_string()
    } else {
        parts.join(" | ")
    }
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

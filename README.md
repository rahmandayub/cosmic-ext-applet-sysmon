# Cosmic Sysmon

![Cosmic Sysmon Panel](screenshots/panel.png)

A compact system monitor applet for the [COSMIC][cosmic] desktop that shows
**CPU usage**, **RAM consumption**, **GPU utilization / VRAM / temperature** and
**network download / upload** speeds directly in the panel.

A click on the panel line opens a checklist popup where you can toggle each
metric and pick the network interface to monitor.

## Features

- Live CPU usage (overall %).
- Live RAM used / total.
- GPU utilization, VRAM used / total, and temperature (NVIDIA via NVML, AMD
  and Intel via sysfs).
- Network download and upload speed, with per-interface selection.
- Configurable refresh interval; configurable per-metric visibility.

## Getting Started

Install [`just`][just] and `cargo`, then run one of the following recipes:

```sh
just build-release   # build the applet
just run             # build and run it
just install         # install to /usr
```

After installation, add the applet to your COSMIC panel from the COSMIC
settings.

## Configuration

The applet persists its settings via `cosmic-config`. The configuration
includes:

- `refresh_interval_ms` — how often to refresh sensor data (default `2000`).
- `show_cpu` / `show_memory` / `show_gpu` / `show_network` — toggles for each
  metric.
- `network_interface` — the network interface to monitor (`None` = auto-pick
  the primary non-loopback interface).

## Building

```sh
just build-release
```

The binary is written to `target/release/cosmic-ext-applet-sysmon`.

[cosmic]: https://github.com/pop-os/cosmic
[just]: https://github.com/casey/just
[libcosmic]: https://github.com/pop-os/libcosmic

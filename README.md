<div align="center">

![BASpark](https://socialify.git.ci/TurkeyC/BASpark-Wayland/image?description=1&descriptionEditable=Blue%20Archive%20Style%20Particle%20Effect%20for%20Linux/Wayland&font=Inter&forks=1&issues=1&logo=https%3A%2F%2Fraw.githubusercontent.com%2FDoomVoss%2FBASpark%2Fmain%2Fassets%2Flogo.png&name=1&pattern=Diagonal%20Stripes&pulls=1&stargazers=1&theme=Auto)

[![GitHub stars](https://img.shields.io/github/stars/TurkeyC/BASpark-Wayland?style=social)](https://github.com/TurkeyC/BASpark-Wayland/stargazers)
[![GitHub license](https://img.shields.io/github/license/TurkeyC/BASpark-Wayland)](https://github.com/TurkeyC/BASpark-Wayland/blob/main/LICENSE)
[![GitHub issues](https://img.shields.io/github/issues/TurkeyC/BASpark-Wayland)](https://github.com/TurkeyC/BASpark-Wayland/issues)

[Download](https://github.com/TurkeyC/BASpark-Wayland/releases/latest) | [Bug Report](https://github.com/TurkeyC/BASpark-Wayland/issues/new) | [Feature Request](https://github.com/TurkeyC/BASpark-Wayland/issues/new)

</div>

---

# BASpark-Wayland

> **A Linux/Wayland Native Desktop Particle Effect Tool:** Reconstructing the iconic Blue Archive UI interactive visual dynamics using Rust + tiny-skia.

**BASpark-Wayland** is a lightweight, native Linux desktop particle effect tool that precisely reproduces the clicking and trail animations from the game *Blue Archive*. It runs as a transparent Wayland overlay using `wlr-layer-shell` and renders particles with a pure-CPU 2D vector engine.

This is a **Linux/Wayland native port** of the original [BASpark](https://github.com/DoomVoss/BASpark) by DoomVoss (Windows WPF + WebView2 version).

---

## Features

* **Authentic Visual Fidelity** — Three particle subsystems (click waves, spinning sparks, mouse trails) faithfully recreate the Blue Archive look and feel.
* **Wayland Native** — Uses `zwlr-layer-shell-v1` for a transparent overlay, `wl_pointer` for input capture, and `zwlr_virtual_pointer_v1` for click-through forwarding.
* **CPU-Only Rendering** — Built on `tiny-skia`, a pure Rust 2D vector graphics library. No GPU or WebView required.
* **Zero Overhead When Idle** — The renderer hibernates completely when there are no active particles.
* **Fully Configurable** — Particle color, scale, opacity, speed, trail behavior, refresh rate, and more via TOML config files.
* **CLI-Driven** — Daemon-style lifecycle with `start`, `stop`, `status`, `config`, and `reload` commands.

---

## Getting Started

### System Requirements

* **Operating System:** Linux with a Wayland compositor supporting `wlr-layer-shell` (e.g., Sway, Hyprland, River, Wayfire)
* **Compositor Features:** `wlr-layer-shell-unstable-v1`, `zwlr-virtual-pointer-v1`

### Installation

#### From Source

```bash
git clone https://github.com/TurkeyC/BASpark-Wayland.git
cd BASpark-Wayland
cargo build --release
sudo cp target/release/baspark /usr/local/bin/
```

#### From Releases

Download the latest binary from the [Releases page](https://github.com/TurkeyC/BASpark-Wayland/releases/latest), then:

```bash
chmod +x baspark
sudo mv baspark /usr/local/bin/
```

### Usage

```bash
# Start the overlay daemon
baspark start

# Stop it
baspark stop

# Check if running
baspark status

# View current configuration
baspark config --show

# Customize particle color
baspark config --color "255,100,100"

# Change effect scale
baspark config --scale 2.0

# Reload config without restarting
baspark reload
```

Configuration is stored at `~/.config/BASpark/config.toml`.

---

## Building from Source

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run directly
cargo run -- start
```

### Dependencies

* Rust 2021 edition (1.70+)
* Wayland development libraries (`libwayland-dev` or equivalent)
* No other runtime dependencies — the binary is fully self-contained.

---

## Configuration Reference

| Option | Default | Description |
|--------|---------|-------------|
| `particle_color` | `"45,175,255"` | RGB color string `"R,G,B"` |
| `is_effect_enabled` | `true` | Master toggle |
| `effect_scale` | `1.5` | Particle size multiplier (0.5–3.0) |
| `effect_opacity` | `1.0` | Global opacity (0.1–1.0) |
| `use_linked_speed` | `true` | Link trail & click speed to single value |
| `effect_speed` | `1.0` | Speed when linked (0.2–3.0) |
| `trail_speed` | `1.0` | Trail animation speed (0.2–3.0) |
| `click_speed` | `1.0` | Click animation speed (0.2–3.0) |
| `trail_refresh_hz` | `40` | Render frame rate (10–240 Hz) |
| `enable_always_trail` | `false` | Show trail without holding mouse button |
| `click_trigger` | `"left"` | Mouse button to trigger effects (`left`/`right`/`both`) |
| `filter_mode` | `"disabled"` | Process filter (`disabled`/`blacklist`/`whitelist`) |
| `filter_processes` | `[]` | Process names for filter |
| `hide_in_fullscreen` | `true` | Auto-disable on fullscreen apps |
| `show_on_desktop` | `true` | Enable on desktop |
| `autostart` | `false` | Start with session |

---

## Project Architecture

```
src/
├── main.rs          # CLI entry point (clap)
├── app.rs           # Main loop, signal handling, frame timing
├── config.rs        # TOML config load/save
├── input.rs         # Input event types
├── overlay.rs       # Wayland wlr-layer-shell overlay + input capture
├── autostart.rs     # XDG autostart management
└── renderer/
    ├── mod.rs       # ParticleEngine — orchestrates all subsystems
    ├── trail.rs     # Mouse trail rendering (gradient segments)
    ├── wave.rs      # Click wave/ring rendering
    ├── spark.rs     # Spark particle rendering (rotating triangles)
    ├── pool.rs      # Generic object pool
    └── dirty_rect.rs# Dirty rectangle tracking for partial updates
```

## Contributors

<a href="https://github.com/TurkeyC/BASpark-Wayland/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=TurkeyC/BASpark-Wayland" />
</a>

### Contributing

Pull requests are welcome! Please ensure your code follows the existing style and passes `cargo build`.

---

## Disclaimers

* This is a non-commercial, fan-made tribute project. Commercial redistribution is prohibited.
* Visual style is inspired by Nexon / Yostar's *Blue Archive*. All copyrights belong to their respective owners.
* This software is provided "as-is" without warranty.

---

## License

MIT License — see the [LICENSE](./LICENSE) file for details.

Original BASpark (Windows version) by [DoomVoss](https://github.com/DoomVoss/BASpark).

<div align="center">
  Made with ❤️
</div>

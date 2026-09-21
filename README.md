<p align="center">
  <h1 align="center">DriftGlide</h1>
  <p align="center">
    <b>Gesture navigation bar & Circle to Search for Linux Wayland</b>
  </p>
</p>

<p align="center">
  <a href="README.md"><b>English</b></a> | <a href="README_RU.md"><b>Русский</b></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/platform-Linux%20Wayland-blue" alt="Platform">
  <img src="https://img.shields.io/badge/language-Rust-orange" alt="Language">
  <img src="https://img.shields.io/badge/rendering-tiny--skia-green" alt="Renderer">
  <img src="https://img.shields.io/badge/AI-Gemini%20Vision-purple" alt="AI">
  <img src="https://img.shields.io/badge/license-GPL--3.0-blue" alt="License">
</p>

---

**DriftGlide** is a navigation daemon for Wayland compositors providing a fluid gesture navigation bar (pill), an integrated Task Switcher, and **Circle to Search** with Google Gemini Vision AI integration to analyze captured screen regions.

It serves as a companion daemon for the [driftwm](https://github.com/wwmaxik/driftwm) window manager and communicates with it via an IPC socket.

## ✨ Features

### 🏠 Gesture Navigation Bar
- Interactive bottom navigation bar with fluid physics
- **Swipe Left / Right** — fast window switching (`focus_prev` / `focus_next`)
- **Swipe Up** — open the Task Switcher
- **Long Press** — activate Circle to Search
- Full support for touchscreens, touchpads, and mouse
- Smooth damped harmonic spring physics with inertia

### 🔲 Task Switcher
- Full-screen overlay displaying open client windows
- Window list queried directly from driftwm via IPC
- Click window card to focus and close switcher
- Automatic idle dismissal timeout

### 🔍 Circle to Search + Gemini Vision
- Seamless screen capture (`zwlr_screencopy_v1` with `spectacle` / `grim` fallbacks)
- Freeform lasso selection with finger or mouse cursor
- Instant fragment crop with automatic clipboard copy (`wl-copy`)
- Floating Gemini Vision chat window:
  - Full Markdown rendering (headers, inline code, fenced code blocks, lists, quotes, dividers)
  - Interactive multi-turn conversation history
  - Screen crop preview thumbnail
  - Draggable window header
  - Minimizable to floating bubble
  - Smooth mouse wheel scrolling and slim scrollbar with auto-scroll while selecting
  - Model selection via interactive chips (Gemini 3.1 Flash Lite / Gemini 3.5 Flash Lite)
  - In-app API key configuration

### ⌨️ Full Keyboard & Selection Support
- `Ctrl+A` / `Ctrl+Ф` — select all (in input field or chat response)
- `Ctrl+C` / `Ctrl+С` — copy selected text (or full response)
- `Ctrl+X` / `Ctrl+Ч` — cut text
- `Ctrl+V` / `Ctrl+М` — paste from clipboard
- `Shift+←/→/Home/End` — keyboard text selection
- Double click to select word, triple click to select all
- Mouse drag text selection in input field and chat response with auto-scrolling

## 📋 Requirements

- **Linux** with a **Wayland** compositor
- Compositor support for `wlr-layer-shell-v1` (Sway, Hyprland, wlroots-based, KDE Plasma 6+)
- **Rust** ≥ 1.70 & `cargo`
- `wl-copy` / `wl-paste` (for clipboard integration)
- `spectacle` or `grim` (for screen capture fallback if `zwlr_screencopy_v1` is unavailable)

## 🚀 Build & Installation

### Using Makefile (Recommended)

```bash
# Clone repository
git clone https://github.com/wwmaxik/driftglide.git
cd driftglide

# Build release binary
make build

# Install to /usr/local/bin (requires sudo)
sudo make install
```

Custom prefix installation:
```bash
make PREFIX=/usr install
```

### Manual Cargo Build

```bash
cargo build --release
cp target/release/driftglide /usr/local/bin/
```

## 🔄 Autostart in driftwm

To launch DriftGlide automatically with **driftwm**, add it to your `autostart` list in `~/.config/driftwm/config.toml`:

```toml
autostart = ["driftglide"]
```

Or along with other services:

```toml
autostart = ["waybar", "driftglide"]
```

## ⚙️ Configuration & Environment

| Variable | Description |
|---|---|
| `GEMINI_API_KEY` | Google Gemini API key (or saved in `~/.config/driftglide/gemini_key`) |
| `DRIFTWM_SOCKET` | Path to driftwm IPC socket (detected automatically) |
| `RUST_LOG` | Logging verbosity (`driftglide=info`, `driftglide=debug`) |

### Gemini API Key Setup
You can provide your Gemini API key in three ways:
1. Environment variable: `export GEMINI_API_KEY="your-key"`
2. Configuration file: store it in `~/.config/driftglide/gemini_key`
3. UI: Click ⚙ in the Gemini window and paste your key

## 🤝 Contributing

Contributions are warmly welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) (or [CONTRIBUTING_RU.md](CONTRIBUTING_RU.md)) for guidelines, development workflow, and coding standards.

Please review our [Code of Conduct](CODE_OF_CONDUCT.md).

## 📄 License

This project is licensed under the **GNU General Public License v3.0** (GPL-3.0) — see the [LICENSE](LICENSE) file for details.

---

<p align="center">
  <i>Built with ❤️ for DriftWM</i>
</p>

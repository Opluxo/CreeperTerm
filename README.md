# CreeperTerm

A modern, GPU-accelerated terminal emulator built with Rust and [iced](https://github.com/iced-rs/iced).

## Features

- **Cross-platform** - Runs on Windows, macOS, and Linux
- **GPU rendering** - Smooth text rendering powered by iced's wgpu backend
- **Multi-tab** - Open multiple terminal sessions in tabs
- **Theme system** - Built-in themes (Default, Dracula) with hex color support
- **Full VT100/xterm compatibility** - ANSI escape sequences, 256-color, true color (RGB)
- **PTY support** - Native pseudo-terminal integration via `portable-pty`
- **SSH client** - Connect to remote servers via password or key authentication
- **Plugin system** - Extensible via `.ctp` plugin packages
- **Configurable** - TOML-based configuration with sensible defaults

## Requirements

- Rust 1.75+
- System dependencies:
  - **Linux**: `libxcb`, `libxkbcommon`, `libfontconfig`, `libfreetype`
  - **macOS**: Xcode command line tools
  - **Windows**: No additional dependencies

### Linux (Ubuntu/Debian)

```bash
sudo apt install libxcb-shape0-dev libxcb-xfixes0-dev libxcb-render0-dev \
    libxcb-xkb-dev libxkbcommon-dev libxkbcommon-x11-dev \
    libfontconfig1-dev libfreetype6-dev
```

### Linux (Arch)

```bash
sudo pacman -S libxcb libxkbcommon xkbcommon libfontconfig freetype2
```

## Build & Run

```bash
# Clone
git clone https://github.com/Opluxo/CreeperTerm.git
cd CreeperTerm

# Build
cargo build --release

# Run
cargo run --release
```

## Configuration

Configuration file is located at:

| Platform | Path |
|----------|------|
| Linux | `~/.config/creeper-term/config.toml` |
| macOS | `~/Library/Application Support/creeper-term/config.toml` |
| Windows | `%APPDATA%\creeper-term\config.toml` |

### Example Config

```toml
[general]
shell = "/bin/bash"
window_title = "CreeperTerm"

[appearance]
theme = "default"
font_family = "Fira Code"
font_size = 14
window_width = 1200
window_height = 800
opacity = 1.0

[terminal]
scroll_buffer_size = 10000
cursor_style = "Block"
cursor_blink = true

[ssh]
default_port = 22
keep_alive_interval = 60

[plugins]
enabled = true
```

### Color Formats

Colors support multiple formats:

```toml
# Named colors
foreground = "bright-white"
background = "#1e1e1e"

# Hex RGB
cursor = "#ffffff"

# Standard 16 colors
color = "red"
```

## Plugin System

Plugins use the `.ctp` format (gzipped tar with `manifest.toml`).

### Plugin Manifest

```toml
[plugin]
name = "my-plugin"
version = "0.1.0"
description = "A CreeperTerm plugin"
author = "Your Name"
```

### Commands

```bash
# Install a plugin
cp my-plugin.ctp ~/.local/share/creeper-term/plugins/

# Plugins are auto-discovered on startup
```

## Project Structure

```
CreeperTerm/
├── src/
│   ├── main.rs              # Entry point
│   ├── app.rs               # Application core (iced update/view loop)
│   ├── terminal/
│   │   ├── buffer.rs        # Terminal screen buffer
│   │   ├── parser.rs        # ANSI/VT escape sequence parser
│   │   ├── pty.rs           # Pseudo-terminal wrapper
│   │   └── state.rs         # Terminal state management
│   ├── ui/
│   │   ├── tab_bar.rs       # Tab bar widget
│   │   └── theme.rs         # Theme definitions & color mapping
│   ├── ssh/
│   │   └── mod.rs           # SSH client (ssh2)
│   ├── plugin/
│   │   └── loader.rs        # Plugin manager (.ctp packages)
│   └── config/
│       └── settings.rs      # TOML configuration
├── config/
│   └── default.toml         # Default configuration
└── Cargo.toml
```

## Keyboard Shortcuts

| Action | Shortcut |
|--------|----------|
| Copy | `Ctrl+Shift+C` |
| Paste | `Ctrl+Shift+V` |
| New Tab | `Ctrl+Shift+T` |
| Close Tab | `Ctrl+Shift+W` |
| Next Tab | `Ctrl+Tab` |
| Previous Tab | `Ctrl+Shift+Tab` |
| Scroll Up | `Shift+Page Up` |
| Scroll Down | `Shift+Page Down` |

## License

GNU General Public License v3.0. See [LICENSE](LICENSE) for details.

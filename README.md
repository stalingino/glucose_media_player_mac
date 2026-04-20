<div align="center">
  <a href="https://glucose.media"><img src="static/logo-dark.svg" alt="Glucose Media Player" width="400"></a>

_A Sleek & Lightweight VLC Alternative with On-Device AI Subtitle<br>_


[![License: MPL](https://img.shields.io/badge/License-MPL%20v2.0-1a1a1a?style=for-the-badge&logo=opensourceinitiative&logoColor=white&labelColor=0a0a0a)](https://joinup.ec.europa.eu/collection/eupl/eupl-text-eupl-12)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-1a1a1a?style=for-the-badge&logo=tauri&logoColor=white&labelColor=0a0a0a)](https://tauri.app)
[![Svelte](https://img.shields.io/badge/Frontend-Svelte-1a1a1a?style=for-the-badge&logo=svelte&logoColor=white&labelColor=0a0a0a)](https://svelte.dev)
[![Rust](https://img.shields.io/badge/Backend-Rust-1a1a1a?style=for-the-badge&logo=rust&logoColor=white&labelColor=0a0a0a)](https://rust-lang.org)

[![Version](https://img.shields.io/github/v/release/stalingino/glucose_media_player_mac?style=for-the-badge&labelColor=0a0a0a&color=1a1a1a&label=Version)](https://github.com/stalingino/glucose_media_player_mac/releases)
[![Downloads](https://img.shields.io/github/downloads/stalingino/glucose_media_player_mac/total?style=for-the-badge&labelColor=0a0a0a&color=1a1a1a)](https://github.com/stalingino/glucose_media_player_mac/releases)
[![Platform](https://img.shields.io/badge/Platform-macOS-1a1a1a?style=for-the-badge&labelColor=0a0a0a)](https://github.com/stalingino/glucose_media_player_mac/releases)

</div>

---

## ✨ Features

### 🎬 Cinematic Mode
Enjoy your media with a beautifully blurred background and centered content for truly immersive viewing.

### 🖼️ Universal Media Support
- **Videos**: MP4, MKV, AVI, MOV, WebM, WMV, FLV, M4V
- **Subtitles**: SRT, VTT, ASS, SSA, SUB

### 🎯 Minimal by Design
No clutter, no distractions. Just your content and elegant controls that appear when you need them.

### ⚡ Blazingly Fast
Built with Rust and Tauri for native performance with a tiny footprint.

### 🎮 Keyboard-First
Complete keyboard navigation for power users who value efficiency.

### 📂 Smart Gallery
Automatically scans and displays your recent videos in a beautiful grid layout.

### 🎨 Modern Interface
- Frameless, transparent window design
- Smooth animations and transitions
- Audio output device selection
- Volume control with visual feedback
- Timeline scrubbing with video preview
- Fullscreen and cinematic viewing modes

---

## 📥 Installation

### Pre-built Binaries

Download the latest release for your platform:

- **macOS**: `Glucose_macOS.dmg` (Universal — Apple Silicon & Intel)

### Build from Source

#### Prerequisites

- [Node.js](https://nodejs.org/) (v18 or later)
- [pnpm](https://pnpm.io/) (recommended) or npm
- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- Platform-specific requirements:
  - **macOS**: Xcode Command Line Tools (`xcode-select --install`)

#### Building

```bash
# Clone the repository
git clone https://github.com/stalingino/glucose_media_player_mac.git
cd glucose_media_player_mac

# Install dependencies
pnpm install

# Run in development mode
pnpm tauri:dev

# Build for production
pnpm tauri:build
```

The built application will be available in `src-tauri/target/release/bundle/`.

---

## 🚀 Usage

### Opening Media Files

glucose offers three convenient ways to open your media:

1. **Drag & Drop** — Simply drag a video or image file into the glucose window
2. **File Dialog** — Click the "Open Video" button in the gallery screen
3. **File Association** — Set glucose as your default media player and open files directly from your file explorer
4. **Recent Gallery** — Browse and play recently accessed videos from the home screen

### Keyboard Shortcuts

#### Playback Controls (Video Mode)
| Key | Action |
|-----|--------|
| `Space` or `K` | Play/Pause |
| `←` | Skip backward 5 seconds |
| `→` | Skip forward 5 seconds |
| `↑` | Increase volume |
| `↓` | Decrease volume |
| `M` | Mute/Unmute |
| `C` or `S` | Toggle subtitles on/off |
| `F` | Toggle cinematic/fullscreen mode |

#### Navigation
| Key | Action |
|-----|--------|
| `Backspace` | Return to gallery |
| `Escape` | Close application |
| `Arrow Keys` | Navigate gallery (when in home screen) |
| `Enter` | Open selected video from gallery |

### Advanced Features

- **Subtitle Support**: 
  - Automatically loads .srt files in the same folder as the video
  - Manually load subtitle files from anywhere
  - Toggle subtitles on/off during playback (C/S keys)
- **Timeline Scrubbing**: Hover over the progress bar to see video preview thumbnails
- **Audio Device Selection**: Click the audio device icon to switch between output devices
- **Volume Popup**: Click the volume icon for a vertical volume slider
- **Cinematic Mode**: Enjoy videos with an elegant blurred background
- **Fullscreen Mode**: Press `F` to toggle fullscreen for maximum immersion

---

### Available Scripts

```bash
# Development
pnpm dev              # Start Vite dev server
pnpm tauri:dev        # Start Tauri in dev mode with hot reload

# Code Quality
pnpm check            # Type-check Svelte/TypeScript code
pnpm check:watch      # Type-check in watch mode
pnpm tauri:check      # Check Rust code (cargo check)

# Building
pnpm build            # Build frontend only
pnpm tauri:build      # Build complete application
pnpm preview          # Preview production build locally
```

## 📄 License

Mozilla Public License 2.0 (MPL-2.0)

This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

Copyright (c) 2025 glucose

---

## 🙏 Acknowledgments

- Built with [Tauri](https://tauri.app/) and [Svelte](https://svelte.dev/)
- Inspired by minimalist design principles
- Thanks to all contributors and users

---

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/stalingino/glucose_media_player_mac/issues)
- **Discussions**: [GitHub Discussions](https://github.com/stalingino/glucose_media_player_mac/discussions)

---

<div align="center">
  <p>Made by the maker of <a href="https://github.com/rudi-q/leed_pdf_viewer">LeedPDF</a></p>
  <p>
    <a href="https://github.com/stalingino/glucose_media_player_mac">GitHub</a> •
    <a href="https://github.com/stalingino/glucose_media_player_mac/issues">Report Bug</a> •
    <a href="https://github.com/stalingino/glucose_media_player_mac/discussions">Request Feature</a>
  </p>
</div>

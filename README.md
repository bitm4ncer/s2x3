<p align="center">
  <a href="https://bitm4ncer.github.io/s2x3/">
    <img src="docs/assets/icon.png" alt="s2X3" width="120">
  </a>
</p>

<h1 align="center">s2X3</h1>

<p align="center">
  <strong>A clean launcher and installer for the native SSX 3 port on PlayStation 2.</strong><br>
  Your disc, your settings, one place.
</p>

<p align="center">
  <a href="LICENSE"><img alt="License: GPL-3.0-or-later" src="https://img.shields.io/badge/license-GPL--3.0--or--later-blue"></a>
  <a href="https://github.com/bitm4ncer/s2x3/releases/latest"><img alt="Linux alpha" src="https://img.shields.io/badge/Linux-alpha-f26a1b?logo=linux&amp;logoColor=white"></a>
  <a href="https://github.com/bitm4ncer/s2x3/releases/latest"><img alt="Windows alpha" src="https://img.shields.io/badge/Windows-alpha-f26a1b?logo=windows&amp;logoColor=white"></a>
  <a href="https://github.com/bitm4ncer/s2x3/releases/latest"><img alt="macOS planned" src="https://img.shields.io/badge/macOS-planned-555555?logo=apple&amp;logoColor=white"></a>
  <a href="https://github.com/bitm4ncer/s2x3/releases/latest"><img alt="Status: alpha" src="https://img.shields.io/badge/status-alpha-f26a1b"></a>
  <a href="https://github.com/andersfischernielsen/ssx3-decomp"><img alt="Engine: ssx3-decomp" src="https://img.shields.io/badge/engine-ssx3--decomp-123a63?logo=rust&amp;logoColor=white"></a>
</p>

> **You need your own SSX 3 PS2 USA ISO.** s2X3 ships no game data and no ISO.
> Use a legally owned copy of the game. [Search DuckDuckGo](https://duckduckgo.com/?q=SSX+3+USA+PS2+ISO)
> for the USA disc image (`SLUS_207.72`).

---

s2X3 is a desktop launcher for [ssx3-decomp](https://github.com/andersfischernielsen/ssx3-decomp),
the native Rust port of SSX 3 for PlayStation 2. It detects your installation,
edits the engine settings through a schema, starts the engine as a child
process and keeps paths, saves and logs in one place.

It is not an emulator, not a recompiler and not a game distribution. It ships
no engine code and no game data. You bring your own USA disc image.

![s2X3 launcher](docs/assets/launcher.png)

[**Website and screenshots →**](https://bitm4ncer.github.io/s2x3/)

## Features

- **Disc detection**: finds the USA disc image (`SLUS_207.72`) and verifies it before launch.
- **Settings that stay**: every engine option is generated from a schema, written to the engine config and merged with changes made in the in game overlay (F10).
- **One click play**: starts the native port fullscreen, with Xbox button prompts, 16:9 and 60 to 240 Hz support.
- **Saves**: export and import the PS2 memory card with timestamped backups.
- **Engine setup**: download and build `ssx3-decomp` from the app, plus an update check when the repository moves forward.
- **Trick spin controls**: cross pad, left stick or right stick, with air gating so acceleration never triggers a flip by accident.
- **Logs**: engine output and build output are available inside the app.
- **No game data**: no ISO, no extracted assets and no generated code in the repository.

## Requirements

### Systems

| System | Launcher | Engine |
| --- | --- | --- |
| **Windows 10/11 (64-bit)** | Alpha installer; WebView2 is installed by the bootstrapper. | DirectX 12 or Vulkan, with the current NVIDIA/AMD/Intel driver. |
| **Linux x86_64** | Alpha AppImage, `.deb` or `.rpm`; WebKitGTK 4.1 is required. | Vulkan through Mesa (RADV/ANV) or the proprietary NVIDIA driver. |
| **macOS** | Planned. | Metal (the engine builds; the launcher does not ship yet). |

### GPUs

The renderer is `wgpu`; it uses **DirectX 12 on Windows** or **Vulkan 1.1+ on Linux** (Vulkan is also available on Windows). The common families below run with current drivers. This is a compatibility list, not a fixed performance promise: the project is tested on Intel Iris Xe, which reaches 60 fps with the heavy effects off.

| Vendor | Windows | Linux |
| --- | --- | --- |
| **NVIDIA** | GeForce GTX 750 Ti, GTX 900 / 10 / 16 series, RTX 20 / 30 / 40 / 50 series; equivalent Quadro | The same GPUs with the current proprietary NVIDIA driver (535 or newer). |
| **AMD** | Radeon RX 400 / 500 / 5000 / 6000 / 7000 / 9000 series, Vega, and Ryzen APUs with Radeon graphics | Mesa RADV/AMDVLK or the AMD proprietary driver; the same families. |
| **Intel** | Iris Xe, Arc A/B, UHD 620 / 630 / 770; older HD 530/630 may work but are not recommended | Mesa ANV; the same families. |

- Install the current GPU driver; the Microsoft Basic Display Adapter has no Vulkan/DX12.
- Linux needs the Vulkan loader and a current Mesa or NVIDIA driver. X11 and Wayland are supported.
- WSL is not supported for the engine.
- Integrated GPUs and older cards may need the heavy graphics effects off to hold 60 fps.

### Other requirements

- Your own **USA** disc image of SSX 3 for PlayStation 2 (`SLUS_207.72`). No game data is included.
- **Rust (stable) and Python 3** to build the engine from source; about **7 GB** free disk.
- Linux build packages: `pkg-config`, `libasound2-dev`, `libudev-dev`, plus WebKitGTK 4.1 for the launcher.
- Windows build: the MSVC toolchain (not WSL).
- Linux is the daily-driver. Windows and Linux alpha installers are published in [Releases](https://github.com/bitm4ncer/s2x3/releases/latest); macOS is planned.

## Quick start

Build the CLI and the desktop app:

```sh
git clone https://github.com/bitm4ncer/s2x3.git
cd s2x3
cargo build --release
./target/release/s2x3 detect
```

```sh
cd apps/desktop
npm install
npm run tauri dev
```

The app can download and build the engine for you. To do it by hand:

```sh
git clone https://github.com/andersfischernielsen/ssx3-decomp.git ~/Projects/ssx3/ssx3-decomp
cd ~/Projects/ssx3/ssx3-decomp
SSX3_ISO="/path/to/SSX 3 (USA).iso" python3 tools/setup.py
cargo build --profile game -p ssx3
```

Point s2X3 at the engine folder and the ISO in **Change paths**, then press
**play**.

## Status

Alpha, used daily on Linux. The launcher, settings, saves and engine setup
work. See [ROADMAP.md](ROADMAP.md) for the phases and [AGENTS.md](AGENTS.md)
for the split between launcher work and upstream engine fixes.

Planned:

- menus and intro video fill for 4:3 front end screens
- performance presets for integrated GPUs
- Tricky courses as plugins
- Tricky audio dub
- web companion, no game code hosted
- LAN and online multiplayer later

## Legal

SSX is a trademark of Electronic Arts. s2X3 is an unofficial fan project,
not affiliated with or endorsed by EA. You need your own copy of the game. The
repository contains no game data, no ISO, no extracted textures and no
recompiled game code. Controller glyphs are original inline SVG; no third-party
button art is bundled. The display face, Tomorrow (SIL OFL 1.1), is bundled
with the source.

## License

[GPL-3.0-or-later](LICENSE). The engine is MIT licensed by its authors. Keep
the engine notices when you redistribute it.

## Credits

- [ssx3-decomp](https://github.com/andersfischernielsen/ssx3-decomp) by [Anders Fischer Nielsen](https://github.com/andersfischernielsen), MIT.
- [Tauri](https://tauri.app) for the desktop shell.
- [Tomorrow](https://fonts.google.com/specimen/Tomorrow) display face by the Tomorrow Project Authors, SIL OFL 1.1.
- [Silkscreen](https://fonts.google.com/specimen/Silkscreen) pixel face by Jason Kottke, SIL OFL 1.1.
- The SSX 3 community for testing, saves and feedback.

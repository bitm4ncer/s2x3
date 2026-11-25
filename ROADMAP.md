# s2X3 Roadmap

s2X3 is a launcher and installer for SSX 3 ports. The first engine is the
PlayStation 2 / Rust port (`ssx3-decomp`); the Xbox recomp can follow as a
second adapter.

Guiding rule: **the launcher never contains engine code and never ships game
data.** It detects, configures and starts engines.

## Phases

### Phase 0 - Foundations (done)

- Rust workspace: `s2x3-core` (contract), `s2x3-engines` (adapters),
  `s2x3-cli` (`s2x3`).
- `EngineAdapter` with capabilities, settings schema, detection and launch.
- `ps2-rust` adapter: disc serial check, settings file, memory card, env.
- Config at `~/.config/s2x3/config.toml`.

Acceptance: `s2x3 detect` sees the disc and the engine; `s2x3 launch`
starts a playable game at 60 fps.

### Phase 1 - Desktop launcher v0.1 (done 2026-11-25)

- Tauri app (`apps/desktop`) with a static web UI, packaged for Linux first.
- One screen: game status, settings generated from the adapter schema,
  play button, controller hints with Xbox glyphs.
- Settings are written to the engine file; the launcher stores defaults and
  never overwrites what the engine's own overlay (F10) saved.
- Save game export/import: copies the memory card to/from a file chosen in a
  native dialog, keeps a timestamped backup before importing.
- Engine log tail in the UI.

Acceptance: a fresh user can point at disc + engine, change a setting, press
Play and get into the game; the app never blocks on the engine.

### Phase 2 - Installer

- First-run wizard: choose disc image, verify serial `SLUS_207.72`, show size
  and region, choose save folder.
- Engine setup: clone/pin `ssx3-decomp`, run `tools/setup.py` with
  `SSX3_ISO`, show progress, run the build, verify the binary.
- Disk-space and toolchain checks (Rust, Python).
- Desktop entry and icon (Linux), shortcut (Windows), app bundle (macOS).
- Version pinning and an update check for the launcher and the engine.

Acceptance: from a clean machine with Rust installed, the wizard ends in a
playable game without a terminal.

### Phase 3 - Quality and controller UX

- Fix the depth-readback stalls (`depth of frame N: the readback never
  completed`) on Intel/Mesa; measure frame times, not just fps.
- Controller navigation in the launcher (d-pad, A/B), hot-plug.
- Xbox button prompts **in the game** via the engine's texture replacement
  (`textures/replace/<hash>.png`), shipped as an optional own-art pack.
- Crash/log capture, "copy diagnostics" button.

Acceptance: stable frame pacing on the reference laptop; prompts match the
connected pad.

### Phase 4 - Web companion

- Static site: landing page, screenshots, docs, download.
- Optional companion page that loads a **locally built** `ssx3-web` package;
  no hosted build, no game code on the server.
- Shared UI components with the desktop app where practical.

Acceptance: the site explains the project and never distributes game data.

### Phase 5 - Content and community (later)

Each item is its own project with its own risks:

- **Custom maps / Tricky courses**: install and manage the engine's plugin
  packages; needs a Tricky disc and a plugin builder.
- **SSX Tricky audio dub**: replace the music and the announcer ("Super!",
  trick call-outs) with the Tricky versions, built from the user's own Tricky
  disc. Needs an engine-side audio-replacement hook or a plugin that maps
  tracks and announcer events; the launcher only installs/selects the pack.
  No audio files in the repo or in releases.
- **Board editor**: likely an engine-side feature; the launcher only manages
  packs and previews.
- **Global highscores**: service + client, opt-in, no game data.
- **Multiplayer**: depends on an engine with stable netcode (LAN experiment or
  relay); the launcher provides lobby/server settings, not netcode.

## Working with the engine

Engine bugs are fixed **upstream** in `ssx3-decomp`, never in this repo. See
`AGENTS.md` for the workflow and `docs/upstream/` for the notes per bug.

## Non-goals

- Emulation, BIOS handling, piracy: the user brings their own disc.
- Hosting builds that contain game code.
- Using EA trademarks in the project name or logo.

## Overnight / parked tasks

Work that is defined well enough to run unattended, but is not next.

### Menu and intro-video fill (4:3 → window)

Problem: menus, overlays and the intro movie are 4:3 content with black side
bars baked into the wide frame. Presenter scaling cannot remove them (tried
`SSX3_ASPECT=cover`).

Plan:

1. Engine helper `in_frontend()`: read the pause/frontend flags at
   `gp - 0x6A0` (bit 1 = frontend; `gp` is CPU register 28).
2. Presenter: with `SSX3_ASPECT=cover`, copy the central 4:3 region out of the
   frame (GPU texture copy) and scale it to the window, cropping top/bottom.
   Only while `in_frontend()`; races and their HUD stay untouched.
3. Launcher setting `Menus: 4:3 / fill` (default 4:3), also covering the intro
   movie and loading screens.
4. Acceptance: screenshots of main menu, character/peak select, pause,
   results, lodge/shop and the intro, all filling the width without
   distortion; race HUD unchanged.
5. Upstream branch `s2x3/menu-fill` plus a note in `docs/upstream/`.


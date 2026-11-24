# Slope3 Roadmap

Slope3 is a launcher and installer for SSX 3 ports. The first engine is the
PlayStation 2 / Rust port (`ssx3-decomp`); the Xbox recomp can follow as a
second adapter.

Guiding rule: **the launcher never contains engine code and never ships game
data.** It detects, configures and starts engines.

## Phases

### Phase 0 - Foundations (done)

- Rust workspace: `slope3-core` (contract), `slope3-engines` (adapters),
  `slope3-cli` (`slope3`).
- `EngineAdapter` with capabilities, settings schema, detection and launch.
- `ps2-rust` adapter: disc serial check, settings file, memory card, env.
- Config at `~/.config/slope3/config.toml`.

Acceptance: `slope3 detect` sees the disc and the engine; `slope3 launch`
starts a playable game at 60 fps.

### Phase 1 - Desktop launcher v0.1 (current)

- Tauri app (`apps/desktop`) with a static web UI, packaged for Linux first.
- One screen: game status, settings generated from the adapter schema,
  play button, controller hints with Xbox glyphs.
- Settings are written to the engine file; the launcher stores defaults.
- Launcher log (engine stdout/stderr) available from the UI.

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

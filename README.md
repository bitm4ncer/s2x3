# Slope3

A launcher and installer for SSX 3 ports. Desktop first (PlayStation 2 / Rust
engine), web companion later. It detects what is installed, keeps the settings
in one place and starts the engine as a child process - it never contains
engine code or game data.

Read [`ROADMAP.md`](ROADMAP.md) for the phases and [`AGENTS.md`](AGENTS.md) for
the rules, especially the split between launcher work and upstream engine
fixes.

## Status

Phase 1: the core, the PS2 adapter and the CLI work; the Tauri desktop app is
in `apps/desktop`.

## Layout

```
crates/slope3-core/      models, config, EngineAdapter
crates/slope3-engines/   one module per engine (ps2.rs today)
crates/slope3-cli/       `slope3` - detect, settings, launch (headless)
apps/desktop/            Tauri app: src/ (UI), src-tauri/ (commands)
docs/architecture.md     how adapters and the UI fit together
docs/upstream/           per-bug engine notes for ssx3-decomp
```

## Build

```sh
cargo build                          # core + CLI
./target/debug/slope3 detect
./target/debug/slope3 launch

cd apps/desktop && npm install && npm run tauri dev
```

The engine itself must be built once:

```sh
cd ssx3-decomp
python3 tools/setup.py               # SSX3_ISO=/path/to/SSX 3 (USA).iso
cargo build --profile game -p ssx3
```

## What may be shipped

- Source only. No ISO, no `generated/` code, no RAM dumps, no built WASM
  packages: those contain the game.
- The engine is MIT; Slope3 is GPL-3.0-or-later. Keep the engine's notices.
- "SSX" is an EA trademark. Describe compatibility, never imply endorsement,
  and do not sell it.

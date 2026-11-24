# Peak3

A launcher for SSX 3 ports. One shell for installs, discs, settings, saves and
(one day) multiplayer - desktop now, web as a companion.

Peak3 does **not** emulate or recompile anything itself. It detects what is
installed, keeps the settings in one place, and starts an engine as a child
process. Engines stay independent; the launcher stays useful for the next one.

## Status

Early vertical slice. One engine adapter works end to end:

- `ps2-rust` - the PlayStation 2 / Rust port ([ssx3-decomp](https://github.com/andersfischernielsen/ssx3-decomp)),
  USA disc `SLUS_207.72`.

Planned: `xbox-recomp` ([koenhendriks/SSX3-recomp](https://github.com/koenhendriks/SSX3-recomp)),
plus a web companion that links to a locally built `ssx3-web` (no game data is
ever hosted).

## Layout

```
peak3/
  crates/peak3-core/     models, config, the EngineAdapter contract
  crates/peak3-engines/  one module per engine (ps2.rs today)
  crates/peak3-cli/      `peak3` - detect, settings, launch
  docs/architecture.md   how adapters and the UI fit together
```

## Build

```sh
cargo build
./target/debug/peak3 init \
  --engine-root ~/Projects/ssx3/ssx3-decomp \
  --iso "$HOME/pCloudDrive/.../SSX 3 (USA).iso" \
  --saves ~/Games/SSX3-ps2/saves
./target/debug/peak3 detect
./target/debug/peak3 settings
./target/debug/peak3 launch
```

The engine itself must be built once:

```sh
cd ssx3-decomp
cp "/path/to/SSX 3 (USA).iso" "SSX 3 (USA).iso"
python3 tools/setup.py
cargo build --profile game -p ssx3
```

## What a launcher may and may not ship

- Source only. No ISO, no `generated/` code, no RAM dumps, no built WASM
  packages: those contain the game.
- The engine is MIT; this launcher is GPL-3.0-or-later. Keep the engine's
  notices when redistributing.
- "SSX" is an EA trademark. Describe compatibility, never imply endorsement,
  and do not sell it.

## Name

`Peak3` is also the name of the SSX 3 decompilation project
(`peak3project/ssx3`). Coordinate before publishing, or pick a distinct
launcher name.

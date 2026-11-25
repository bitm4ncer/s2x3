# Architecture

## The split

```
UI (Tauri/web view, or any front end)
        |
        v
s2x3-core      config.toml, models, EngineAdapter
        |
        v
s2x3-engines   ps2.rs  xbox.rs (later)  ...
        |
        v
child process   target/game/ssx3 | launcher.exe | ...
```

Rules:

1. **No engine code is linked.** The launcher writes settings and spawns a
   process. A crash in the engine never takes the UI with it, and the
   launcher can be licensed and versioned on its own.
2. **Capabilities, not special cases.** The UI asks boolean questions
   (plugins? netplay? splitscreen? max tick rate?) and renders fields from
   the engine's settings schema. A new engine must not need UI code.
3. **Settings are data.** Each adapter maps its schema to a file and/or
   environment. S2X3 stores the canonical values in
   `~/.config/s2x3/config.toml` and writes the engine's file before launch.
4. **Saves are a separate service.** v1 imports/exports and backs up a
   per-engine save folder. Cross-engine conversion is a later, separate
   research item - the formats and regions differ.

## Adapters

`EngineAdapter` (see `crates/s2x3-core/src/lib.rs`):

```rust
fn id() -> &'static str;
fn capabilities() -> Capabilities;
fn detect() -> Detection;                 // engine + disc, with notes
fn settings_schema() -> &'static [SettingsField];
fn read_settings() / write_settings();
fn launch(&LaunchConfig) -> Result<Child>;
```

### ps2-rust

- Engine: `ssx3-decomp`, built as `target/game/ssx3` (or `dist/ssx3`).
- Disc: PS2 USA `SLUS_207.72`, 2.8 GB.
- Settings: `saves/ssx3-settings.cfg`, `KEY=VALUE`, environment overrides.
- Saves: `saves/ssx3-mc0.ps2` (one memory card file).
- Launch env: `SSX3_ISO`, `SSX3_MEMCARD`, `SSX3_SETTINGS`, plus the settings.

### xbox-recomp (planned)

- Engine: `SSX3-recomp`, Windows release or a Linux build.
- Disc: extracted PAL data (or NTSC-U with the unmerged PR).
- Settings: `RECOMP_*` environment variables and the launcher's own config.
- Saves: the UDATA folder next to the game data.
- Capabilities: HD textures, 4K, MSAA, splitscreen; no plugins, no netcode.

There is no shared save format between the two engines today. S2X3 will
offer backup/restore per engine and an explicit "export/import" action, not a
silent conversion.

## Web

The browser cannot run the Xbox recomp. The PS2 engine compiles to WASM
(`ssx3-web`), so the web companion is:

- a landing page with screenshots/video and a local-build guide,
- optionally a page that loads a **user-built** WASM package,
- never a hosted build that contains game code.

The same UI components can serve the desktop and the web page; only the
"launch" action differs (spawn a process vs. open a URL).

## Order of work

1. Vertical slice: one adapter, CLI, generic settings (done).
2. UI shell (Tauri) with detect/settings/launch.
3. Xbox adapter, capability-driven UI.
4. Save backup/restore, mods/plugins list.
5. Netplay panel (relay/LAN), once an engine supports it stably.

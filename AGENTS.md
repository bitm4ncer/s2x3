# AGENTS.md

Instructions for anyone (human or agent) working on Slope3.

## What this project is

A launcher/installer for SSX 3 ports. It detects installations, writes
settings, starts engines as child processes and (later) manages mods, saves
and a web companion. It is **not** an emulator, not a recompiler and not a
game distribution.

## Hard rules

1. **No game data.** Never commit an ISO, `orig/`, `generated/` code, RAM or
   VRAM dumps, extracted assets, or built WASM packages. They contain the
   game.
2. **No engine code in this repo.** The engines are external checkouts.
   Never vendor, copy or patch their sources here.
3. **Licenses.** Slope3 is GPL-3.0-or-later. The PS2 engine is MIT; keep its
   notices. The Xbox recomp is GPL-2.0-or-later. Do not add assets with
   unclear licenses; bundled fonts must be OFL.
4. **Trademarks.** "SSX" is an EA trademark. Use it descriptively
   ("for SSX 3"), never in the product name or logo, and never imply
   endorsement.

## Two separate workstreams

Engine issues and launcher issues must never be mixed in one change.

### A. Slope3 (this repo)

UI, installer, adapters, settings, logs, packaging, docs. Bugs that are
caused by the launcher (wrong paths, wrong env, bad defaults, UI) are fixed
here.

### B. Upstream engine (`ssx3-decomp`)

Anything that requires changing the engine - rendering, timing, input, game
logic - is fixed **upstream**, in a separate checkout and a separate PR.

Workflow when a bug shows up while playing:

1. Reproduce and capture evidence: `~/Games/SSX3-ps2/play.log`, the exact
   settings, frame numbers, a screenshot or a save.
2. Write a note in `docs/upstream/YYYY-MM-DD-<slug>.md`: symptoms, repro,
   logs, first hypothesis, affected engine commit.
3. Work in the engine checkout (default
   `~/Projects/ssx3/ssx3-decomp`), on a branch
   `slope3/<slug>`; keep the diff minimal and add a test or a scripted
   scenario where possible.
4. Open a PR against `andersfischernielsen/ssx3-decomp` with the note's
   summary. Do not bundle unrelated changes.
5. The launcher may only rely on merged engine behaviour. If a local build
   needs an unmerged fix, gate it: record the engine commit/PR in the note,
   keep a `[local]` marker, and make the launcher degrade gracefully when the
   fix is absent.
6. Never copy the patched engine code into Slope3 and never make the launcher
   the distribution channel for a patched engine.

## Repo layout

```
crates/slope3-core/      models, config, EngineAdapter
crates/slope3-engines/   one module per engine (ps2.rs today)
crates/slope3-cli/       `slope3` command line (the reference client)
apps/desktop/            Tauri app: src/ (UI), src-tauri/ (commands)
docs/architecture.md     how the pieces fit
docs/upstream/           per-bug engine notes
ROADMAP.md               phases and acceptance criteria
```

Other front ends (web, mobile) live in `apps/` and use `slope3-core` the same
way as the desktop app.

## Conventions

- Rust 2021, `cargo fmt`, warnings clean.
- Keep the adapter contract small: capabilities, schema, detect, settings,
  launch. New features go behind capabilities, not `if engine == ...`.
- UI: SSX 3 look - deep navy, glacier blue, snow white, signal orange,
  angled panels, condensed uppercase headings. Xbox glyphs (A/B/X/Y) are the
  default on desktop; PlayStation glyphs are a setting.
- Settings are data. The UI renders the schema; it must not hard-code fields.
- Every user-visible action needs a failure path with a readable message.
- Commit messages: short imperative subject, body only when it explains why.

## Checks before claiming something works

- `cargo test` and `cargo build` in the workspace.
- `cargo build` in `apps/desktop/src-tauri`.
- Launch the real game through the launcher and confirm the process starts,
  the log has no new errors, and the UI stays responsive.
- For engine changes: the upstream differential tests and the RAM gate.

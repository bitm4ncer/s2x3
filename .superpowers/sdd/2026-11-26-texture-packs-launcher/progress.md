# SDD ledger — plan: docs/superpowers/plans/2026-11-26-texture-packs-launcher.md

Worktree: C:\Users\konta\.local\share\opencode\worktree\b8f924\texture-packs (branch texture-packs, cut from main @ be4d183)
Spec: docs/superpowers/specs/2026-11-26-texture-packs-design.md (authority)
Upstream sibling plan: docs/superpowers/plans/2026-11-26-texture-packs-upstream.md (anderer Agent, Workstream B)

## Pre-flight

- Task 2→3: `import` konsumiert `read_manifest` (Task 2) — Signaturen konsistent (both in textures.rs, same module). ✓
- Task 3/4→5: `migrate_legacy` nutzt `import` (Task 3) und `merge` (Task 4) — Konsistenz beim Schreiben von Task 5 prüfen. ✓ geplant konsistent.
- Task 1→5: ps2.rs konsumiert `launch_replace_dir` + `paths.textures_active()` — Namen konsistent. ✓
- Task 2–5→6/7: CLI/Tauri konsumieren dieselben core-Funktionen — Namen konsistent. ✓
- Ruling (Vorab): „upstream-Bin noch nicht gemerged" → Task 7 `textures_extract` liefert lesbaren Fehler (Spec-Fallback: Laufzeit-Dump bleibt aktiv); keine Blockade der anderen Tasks.

Task 1: start (base e943a94)
Task 1: start (base e943a94)
Task 1: complete (commits e943a94..eee2628, tests: cargo test -p s2x3-core -> 1 passed 0 failed). Ruling: s2x3-core hat kein bestehendes Testmodul (Plan nahm eines an) - neu angelegt; tempfile als workspace dev-dep ergaenzt.
Task 2: complete (commits eee2628..f22285e, tests: cargo test -p s2x3-core -> 6 passed 0 failed)
Task 3: complete (commits 91af369~, tests: cargo test -p s2x3-core -> 12 passed 0 failed). Ruling: zip-9-API (name() -> Result, enclosed_name), Staging entpackt ohne Root-Ordner (Root = id).
Task 5: complete (commits 3e6a676..917d374, tests: cargo test --workspace -> 18 passed 0 failed). Ruling: ps2.rs hat keine Testinfrastruktur und launch() spawned den echten Bin - die Env-Mapping-Logik wird in core getestet (launch_replace_env), die Adapter-Aenderung ist eine Zeile.
Task 6: complete (commits 917d374..b8968b5, tests: cargo test --workspace -> 23 passed 0 failed). Ruling: Extract/Upscale laufen ueber neue core-Helfer run_extract/run_upscale/engine_binary (ps2.rs binary()-Duplikat bis der Adapter sie exponiert); iso_marker mit sha1 (0.11).
Task 7: Ruling: Basis-Commit e943a94 referenziert mod cards, aber cards.rs/card.rs/ Kern-Delta (memcard, active_memcard, pub mod card) sind uncommitted Arbeit des Parallel-Agenten. Ich kopiere die Dateien UNGETRACKT in den Worktree und uebernehme das Kern-Delta uncommitted; meine lib.rs-Commits tragen das Delta mit (im Commit-Message-Notiz) - beim Rebase auf ihren Merge dedupliziert sich das. Cost if wrong: ihr Halbstand landet unter meiner Message; Rebase-Konflikt moeglich.
Task 7: complete (commits b8968b5..ee7d551, tests: cargo test --workspace -> 25 passed; src-tauri --lib -> 5 passed). Ruling siehe Task-7-Eintrag oben; dump_dir jetzt unter paths.textures/dump (nie Engine-Checkout); xbox-buttons-Pack wird per generate_xbox_pack erzeugt (kind=upscale, lokal-only - Atlas enthaelt Spieldaten um die Slots).
Task 9: complete (9d8cb37, tests: cargo test -p s2x3-core -> 24 passed). Ruling: Katalog als JSON (wie maps.json), Validierungstest mit serde_json als core-dev-dep.
Task 9: complete (tests: cargo test -p s2x3-core -> 24 passed 0 failed). Ruling: Katalog als JSON (wie maps.json), serde_json als core-dev-dep; Cargo.lock liegt am Workspace-Root.

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

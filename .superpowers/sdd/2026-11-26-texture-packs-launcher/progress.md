# SDD ledger â€” plan: docs/superpowers/plans/2026-11-26-texture-packs-launcher.md

Worktree: C:\Users\konta\.local\share\opencode\worktree\b8f924\texture-packs (branch texture-packs, cut from main @ be4d183)
Spec: docs/superpowers/specs/2026-11-26-texture-packs-design.md (authority)
Upstream sibling plan: docs/superpowers/plans/2026-11-26-texture-packs-upstream.md (anderer Agent, Workstream B)

## Pre-flight

- Task 2â†’3: `import` konsumiert `read_manifest` (Task 2) â€” Signaturen konsistent (both in textures.rs, same module). âœ“
- Task 3/4â†’5: `migrate_legacy` nutzt `import` (Task 3) und `merge` (Task 4) â€” Konsistenz beim Schreiben von Task 5 prÃ¼fen. âœ“ geplant konsistent.
- Task 1â†’5: ps2.rs konsumiert `launch_replace_dir` + `paths.textures_active()` â€” Namen konsistent. âœ“
- Task 2â€“5â†’6/7: CLI/Tauri konsumieren dieselben core-Funktionen â€” Namen konsistent. âœ“
- Ruling (Vorab): â€župstream-Bin noch nicht gemerged" â†’ Task 7 `textures_extract` liefert lesbaren Fehler (Spec-Fallback: Laufzeit-Dump bleibt aktiv); keine Blockade der anderen Tasks.

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

Task 8: complete (commit 9d8cb37; Verifikation: src-tauri build + Playwright-Mock-Screenshot qa/textures.png, Tab gerendert). Ruling: index.html unveraendert - der Tab wird dynamisch aus TABS gerendert. Ruling: Basis-Commit e943a94 hat kaputte Karten-Bindings (card-export/hash/paste fehlen im HTML, bind() starb, Settings-Panel leer) - auf committete IDs card-share/card-import umgebunden.
Task 9: complete (commit ecd537d, 24->26 Tests).
Task 10: complete (Gates: workspace 28 passed 0 failed, src-tauri 5 passed 0 failed, fmt clean).
Final: fixed hd-upscale enable-after-merge — enable_and_merge core test RED->GREEN (enable aktiviert+priorisiert vor Merge), suite 28+5/33.
Final: fixed resurrection loops — legacy_migrated/generated_xbox Latches, core test migration_latch_prevents_resurrection RED->GREEN, suite gruen.
Final: fixed conflict counts ueber manifest.targets — conflict_hits auf echte Stems, dto test gruen.
Final: fixed priority dead state — enable_and_merge schreibt Priority-Eintrag (erstes aktiviertes Pack gewinnt), getestet.
Final: fixed assetProtocol still — aktiviert mit textures-Scoped-Allowlist + protocol-asset Feature; Ordner-Import-Picker ergaenzt.
Final: minor (deferred): staging .import-<pid>-Kollision bei parallelen Imports im selben Prozess (Tauri-Pool) — Uniquifier.
Final: minor (deferred): 512-MB-Limit summiert deklarierte Groessen; Laufzeit-Cap in der Kopier Schleife fehlt.
Final: minor (deferred): conflicts()-Parameter priority ignoriert (let _ = priority).
Final: minor (deferred): CLI textures extract ohne ISO-Pruefung vor Spawn (Tauri-Pfad prueft) — Paritaet-Polish.
Final: minor (deferred): CLI-Tests setzen S2X3_CONFIG parallel — Flake-Risiko, Mutex/Shared-Temp moeglich.
Final: minor (deferred): engine_binary dupliziert Ps2Rust::binary — sauber: Adapter-Exposition (capabilities-Muster).
Final: minor (deferred): ISO-Merker wird erst NACH generate_xbox_pack gespeichert — seltener Re-Lauf Minuten, wenn Patch failt.
Final: minor (deferred): persistente Statuszeile + Erneut-versuchen-Affordance im Tab fehlt (nur Toasts); textures://conflicts-Event unimplementiert (DTO-Zaehlung ersetzt es).
Final: minor (deferred): Komplett-Verifikations-Commit "feature complete" nicht eigens gesetzt (Gates in Task-10-Zeile belegt).
Merge: main (ea28071) in texture-packs gemergt (fcf0f91). main.js-Konflikt aufgeloest: ihre Save-Manager-UI als Basis, meine Textures-Bloecke re-appliziert (CRLF-Falle: Multi-Zeilen-Needles brauchen \\r\\n). Screenshots: qa-textures-merged.png. Gates gruen. OFFEN: main hat cards.rs/card.rs + core-Delta immer noch NICHT committed - clean checkouts von main bleiben gebrochen; vor dem echten Landing muss der Parallel-Agent die Dateien committen.

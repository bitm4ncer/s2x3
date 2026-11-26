# Texture Packs — Launcher Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** „Textures"-Tab im Launcher: Packs verwalten (Import/Toggle/Merge/Entfernen), Hintergrund-Extraktion bei ISO-Wechsel, HD-Upscale-Job, Xbox-Buttons-Pack als Ablösung des Texreplace-Mechanismus.

**Architecture:** Shared Logik in `s2x3-core` (Modul `textures`), CLI-Parität in `s2x3-cli`, Tauri-Backend `textures.rs` + Frontend-Tab. Merge-Ordner `paths.textures/active` wird bei jedem Toggle neu gebaut; `SSX3_TEX_REPLACE` wird immer explizit gesetzt. Der Offline-Extraktor ist ein upstream-Bin (`ssx3 dump-textures`, eigener Plan `2026-11-26-texture-packs-upstream.md`) — bis dahin läuft die Extraktion über den Laufzeit-Dump (heutiger Mechanismus) und das Feature degradiert lesbar.

**Tech Stack:** Rust 2021, `toml` + `serde` (bereits im Workspace), `zip`-Crate (neu), `sha1`-/`sha2`-Crate (ISO-Merker; prüfen was cards.rs nutzt), Tauri 2, Vanilla-JS-Frontend.

**Spec:** `docs/superpowers/specs/2026-11-26-texture-packs-design.md`

## Global Constraints

- Hard Rules aus `AGENTS.md`: kein Spieldatum im Repo/Releases; keine Engine-Code-Vendorung; keine EA-Marken im Produktnamen.
- Niemals in den Engine-Checkout schreiben (`paths.textures` ist die Ablage).
- `cargo fmt` + warnings clean; `cargo test` + `cargo build` (Workspace) und `cargo build` in `apps/desktop/src-tauri` müssen grün bleiben.
- SSX-3-Look: deep navy, glacier blue, snow white, signal orange, angewinkelte Panels; Xbox-Glyphen-Default.
- UI rendert nur Daten; Felder werden nicht hartkodiert (Settings sind Daten).
- Hash-Namen sind 16 Hex-Ziffern, kleingeschrieben (Engine lowercased Stems, `pack.rs:66`).
- Jede Nutzeraktion braucht eine lesbare Fehlermeldung mit Pfadebene.

## Review Focus

1. **Zip-Slip:** ein ZIP mit `../`-Einträgen darf nichts außerhalb des Pack-Ordners schreiben. Test in Task 3.
2. **Toggle-aus-Garantie:** aktivierte Packs leer → `SSX3_TEX_REPLACE=0` (Engine-Default-Ordner darf nicht durchschlagen). Test in Task 5.
3. **Case-Insensitivität:** `<HASH>.PNG` und gemischte Schreibweise müssen denselben Pack-Eintrag treffen wie kleingeschrieben. Test in Task 3.
4. **ISO gewechselt/verscho­ben:** Merker-Hash weicht ab → erneuter Lauf; ISO weg → lesbarer Fehler, kein Crash beim Start. Test in Task 7.
5. **Alter Engine-Stand ohne `dump-textures`:** Extraction-Button zeigt lesbaren Fehler, Rest des Tabs funktioniert. Test in Task 7.

---

### Task 1: `paths.textures` in der Config

**Files:**
- Modify: `crates/s2x3-core/src/lib.rs` (struct `Paths` ~Z. 182, `resolved()` ~Z. 281)
- Test: in `crates/s2x3-core/src/lib.rs` (bestehendes Testmodul)

**Interfaces:**
- Produces: `Paths.textures: PathBuf`, Methoden `textures_dump()`, `textures_packs()`, `textures_active()` → `PathBuf`.

- [ ] **Step 1: Failing test** — in `resolved()`-Tests: `assert_eq!(c.paths.textures, base.join("textures"));` und `textures_dump() == base.join("textures/dump")` (analog packs/active).
- [ ] **Step 2:** `cargo test -p s2x3-core` → FAIL (Feld fehlt).
- [ ] **Step 3:** Feld `pub textures: PathBuf` + `resolved()`-Default `base.join("textures")` (Muster `texreplace`, Z. 297) + die drei Methoden.
- [ ] **Step 4:** `cargo test -p s2x3-core` → PASS.
- [ ] **Step 5:** Commit: `Add textures paths to config`.

### Task 2: `s2x3-core::textures` — Manifest + Liste

**Files:**
- Create: `crates/s2x3-core/src/textures.rs`
- Modify: `crates/s2x3-core/src/lib.rs` (`pub mod textures;`)

**Interfaces:**
- Produces:
  - `pub enum PackKind { OwnArt, Upscale }` (TOML-Strings `"own-art"`/`"upscale"`)
  - `pub struct PackManifest { pub name: String, pub author: String, pub version: String, pub kind: PackKind, pub engine_commit: Option<String>, pub targets: BTreeMap<String, String> }`
  - `pub struct Pack { pub id: String, pub dir: PathBuf, pub manifest: PackManifest }`
  - `pub struct PackEntry { pub id: String, pub pack: Option<Pack>, pub error: Option<String>, pub texture_count: usize }`
  - `pub fn read_manifest(dir: &Path) -> anyhow::Result<PackManifest>` (liest `manifest.toml`)
  - `pub fn list(packs_dir: &Path) -> Vec<PackEntry>` (nach `id` sortiert; defekte Ordner als Entry mit `error`)
  - Hash-Validierung: Texturen mit Stem ≠ 16 Hex-Ziffern erzeugen `error`-Hinweis im Entry (Daten bleiben unangetastet).

- [ ] **Step 1: Failing tests** (Fixture-Ordner in `tempfile`): gültiges Manifest parst (kind own-art, targets); fehlendes `name` = Fehler mit Zeilenhinweis; unbekannter `kind` = Fehler; `list()` sortiert und zeigt defekten Ordner als `error`-Entry.
- [ ] **Step 2:** `cargo test -p s2x3-core textures` → FAIL.
- [ ] **Step 3:** Implementieren (TOML-Parse mit `toml`+`serde` wie `plugin.toml`-Muster).
- [ ] **Step 4:** PASS. **Step 5:** Commit: `Texture pack manifest and listing`.

### Task 3: Import (ZIP/Ordner) + Entfernen

**Files:**
- Modify: `crates/s2x3-core/src/textures.rs`
- Modify: `crates/s2x3-core/Cargo.toml` (`zip`, falls nicht im Workspace)

**Interfaces:**
- Consumes: `read_manifest` (Task 2).
- Produces: `pub fn import(src: &Path, packs_dir: &Path, overwrite: bool) -> anyhow::Result<String>` (→ id), `pub fn remove(packs_dir: &Path, id: &str) -> anyhow::Result<()>`.

- [ ] **Step 1: Failing tests** — synthetische ZIPs im Test bauen: (a) valides Pack wird nach `packs/<id>/` entpackt, id = Root-Ordnername klein; (b) ZIP mit `../evil`-Eintrag wird abgelehnt (Meldung nennt den Eintrag); (c) ZIP ohne `manifest.toml` abgelehnt; (d) Bestehende id + `overwrite=false` = Fehler, mit `true` = atomarer Austausch (alter Ordner beiseite, neu rein, alt weg); (e) `<HASH>.PNG`-Dateien werden wie kleingeschrieben gezählt.
- [ ] **Step 2:** FAIL. **Step 3:** Implementieren (Entpacken nach `packs/.tmp-*`, Zip-Slip-Guard: kanonisierte Pfade müssen unter Ziel liegen, Größenlimit 512 MB, atomar tauschen wie Maps-Design). **Step 4:** PASS. **Step 5:** Commit: `Texture pack import with zip-slip guard`.

### Task 4: Merge nach `active/`

**Files:**
- Modify: `crates/s2x3-core/src/textures.rs`

**Interfaces:**
- Produces: `pub struct MergeReport { pub files: u32, pub conflicts: Vec<String> }`, `pub fn merge(packs_dir: &Path, active_dir: &Path, enabled: &BTreeMap<String, bool>, priority: &[String]) -> anyhow::Result<MergeReport>`.

- [ ] **Step 1: Failing tests** — zwei Packs mit gleichem Hash: nur das nach `priority` erste landet in `active/`, `conflicts` enthält den Hash; deaktiviertes Pack trägt nichts bei; zweiter `merge`-Lauf nach Toggle-aus entfernt die Dateien (Rebuild immer von leerem `active.tmp`).
- [ ] **Step 2:** FAIL. **Step 3:** Implementieren (Kopieren in `active.tmp/`, atomarer Tausch, Reihenfolge: `priority`-Index, dann id).
- [ ] **Step 4:** PASS. **Step 5:** Commit: `Build active texture pack merge directory`.

### Task 5: Config-State, Launch-Env, Migration

**Files:**
- Modify: `crates/s2x3-core/src/lib.rs` (struct `Config` ~Z. 133), `crates/s2x3-core/src/textures.rs`
- Modify: `crates/s2x3-engines/src/ps2.rs` (~Z. 480)

**Interfaces:**
- Produces:
  - `pub struct TexturesSettings { pub enabled: BTreeMap<String, bool>, pub priority: Vec<String>, pub iso_marker: String }`, Feld `Config.textures` (Default leer).
  - `pub fn launch_replace_dir(config: &Config) -> Option<PathBuf>` — `Some(paths.textures_active())`, wenn dort ≥1 PNG liegt.
  - `pub fn migrate_legacy(config: &mut Config) -> bool` — `ui.buttons=="xbox"` → `textures.enabled["xbox-buttons"]=true` (idempotent); `paths.texreplace` mit PNGs + fehlendem Pack `texreplace-import` → als Pack importiert (Meldung im UI folgt in Task 7).
- ps2.rs: ersetzt Z. 480–481 durch: immer `cmd.env("SSX3_TEX_REPLACE", value)` mit `value = launch_replace_dir(...).map(string) .unwrap_or("0")`.

- [ ] **Step 1: Failing tests** — core: `launch_replace_dir` None bei leerem active, Some bei PNG; `migrate_legacy` idempotent. engines: Launch-Env ist `0` ohne Packs, aktiver Pfad mit Pack (Env über `Command`-Mock/Watch wie bestehende Env-Tests in ps2.rs).
- [ ] **Step 2:** FAIL. **Step 3:** Implementieren. **Step 4:** `cargo test --workspace` PASS. **Step 5:** Commit: `Always-explicit texture replacement env and legacy migration`.

### Task 6: CLI `s2x3 textures …`

**Files:**
- Modify: `crates/s2x3-cli/src/main.rs` (enum `Command` ~Z. 36, Dispatch ~Z. 123)

**Interfaces:**
- Consumes: Tasks 2–5.
- Produces: `Command::Textures { action: TexturesAction }`; `enum TexturesAction { List, Import { path: PathBuf }, Remove { id: String }, Enable { id: String }, Disable { id: String }, Extract, Upscale }`. `Extract`/`Upscale` rufen dieselben Kernfunktionen wie Task 7 (Engine-Bin/Python-Job), Fehlertexte nennen fehlende Binaries namentlich.

- [ ] **Step 1: Failing tests** — `textures enable xbox-buttons` schreibt `config.textures.enabled`; unbekannte id = lesbare Meldung; `list` druckt Tabelle (id, kind, Texturen, Status).
- [ ] **Step 2:** FAIL. **Step 3:** Implementieren (Muster `Saves`-Subcommand). **Step 4:** PASS. **Step 5:** Commit: `Texture pack CLI subcommand`.

### Task 7: Tauri-Backend — Kommandos, Hintergrund-Extraktion, Pack-Generator

**Files:**
- Create: `apps/desktop/src-tauri/src/textures.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs` (`mod textures;` + `generate_handler!`), `apps/desktop/src-tauri/src/texreplace.rs` (Ausgabeziel → Pack)

**Interfaces:**
- Consumes: Tasks 2–5; `texreplace::{patch, ATLASES}` (bestehend, Z. 23/75).
- Produces (alle `#[tauri::command]`, Dto-Muster `cards.rs`):
  - `textures_list() -> Vec<TexturePackDto>` (`id, name, author, version, kind, texture_count, enabled, error, conflict_count`)
  - `textures_import(path: String) -> TexturePackDto` / `textures_remove(id: String)` / `textures_toggle(id: String, enabled: bool)` (jeweils `merge()` danach, Report-Konflikte als Event `textures://conflicts`)
  - `textures_extract(window) -> String` (Status-Text) — ISO-Hash (`sha1`, Datei-Stream) gegen `config.textures.iso_marker`; gleich → „bereits aktuell"; anders → Child-Prozess `<engine-bin> dump-textures --iso <paths.iso> --out <textures_dump()>` (Engine-Bin-Auflösung wie bestehende Build-/Launch-Helfer), Events `textures://extract {state, message}`; bei fehlender Unterstützung des Bins: lesbarer Fehler (Review Focus 5). Erfolg → Marker schreiben, danach ggf. `texreplace::patch(dump, pack_dir)` für `xbox-buttons` anstoßen.
  - `textures_upscale(window) -> TexturePackDto` — Child-Prozess `python tools/upscale-textures.py --in <textures_dump()> --out <packs>/hd-upscale --esrgan <textures/.tools>` (Flags kommen aus dem upstream-Plan; bis dahin: lesbarer Fehler „upstream-Tool fehlt"), Vorab-Checks Python/Pillow/Vulkan (Review Focus), Events `textures://upscale`.
  - `textures_open_folder(id: String)` — packt Pfad in die Zwischenablage/öffnet Explorer je nach bestehendem Muster in `cards.rs`.
- `texreplace.rs`-Änderung: `patch` schreibt nach `paths.textures/packs/xbox-buttons/textures/` statt in `paths.texreplace`; Hook-Aufruf nach Spielende bleibt an gleicher Stelle (in `lib.rs` finden via `texreplace::`); danach `merge()`. Trigger zusätzlich beim Toggle von `ui.buttons` auf xbox.

- [ ] **Step 1: Failing tests** — Marker-Vergleich (gleich/anders/leer), Dto-Bau aus PackEntry inkl. Konflikte; Patch-Ausgabe landet im Pack-Ordner (bestehenden Test in texreplace.rs auf neues Ziel umbiegen).
- [ ] **Step 2:** FAIL. **Step 3:** Implementieren. **Step 4:** `cargo test` in `apps/desktop/src-tauri` PASS. **Step 5:** Commit: `Texture packs backend with background extraction`.

### Task 8: Frontend „Textures"-Tab

**Files:**
- Modify: `apps/desktop/src/index.html`, `apps/desktop/src/main.js`, `apps/desktop/src/style.css`

**Interfaces:**
- Consumes: Task-7-Kommandos (`invoke("textures_list")` …), Events `textures://extract|upscale|conflicts`.
- Produces: Tab `textures` neben bestehenden Sektionen; Karten (Vorschau = `preview/*.png` falls vorhanden, sonst Platzhalter), Toggle, „Ordner öffnen", „HD hochskalieren", „Texturen neu lesen", Statuszeile für Hintergrund-Jobs, Konflikt-Warnung.

- [ ] **Step 1:** Markup + Handler + Styles im SSX-3-Look (Muster: bestehende Karten-Sektion).
- [ ] **Step 2:** `cargo build` in `apps/desktop/src-tauri`, App starten, Durchklicken: Import (ZIP aus Task-3-Testfixture), Toggle, Ordner öffnen; Screenshot nach `qa/textures.png`.
- [ ] **Step 3:** Commit: `Textures tab in the desktop launcher`.

### Task 9: Katalog-Metadaten

**Files:**
- Create: `catalog/textures.json`

**Interfaces:**
- Produces: statischer Metadaten-Index (nur Metadaten — Hard Rule 1) mit dem `xbox-buttons`-Eintrag (`id`, `name`, `author`, `version`, `kind`, `description`, `source_note: "im Launcher gebündelt"`). Kein Loader — den baut das Maps-Feature; der Index wird so entworfen, dass er später erweitert wird.

- [ ] **Step 1:** JSON schreiben (Felder wie Maps-Katalog, ohne `url`, da gebündelt); Validierung per Test in `s2x3-core` (JSON parst, Pflichtfelder je Eintrag).
- [ ] **Step 2:** Commit: `Texture pack catalog metadata`.

### Task 10: Gesamt-Verifikation

- [ ] `cargo fmt && cargo test && cargo build --release` im Workspace; `cargo build` in `apps/desktop/src-tauri`; App starten und Abnahme-Schritte 1–5 der Spec soweit möglich (ohne upstream-Bin: Extraktion = Laufzeit-Dump-Weg) abhaken; Screenshot in `qa/`. Commit: `Texture packs feature complete`.

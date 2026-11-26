# Texture Packs — Upstream Extractor Plan (ssx3-decomp)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Offline-Textur-Extraktion in der Engine: `ssx3 dump-textures --iso <p> --out <dir>` liest die ISO direkt und erzeugt PNGs mit denselben Hash-Namen wie der Laufzeit-Dump — ohne Spielstart.

**Architecture:** Neues Untercommand-Bin im Engine-Workspace, das die bestehenden Dekodier- und Namensfunktionen aus `ssx3-gfx` wiederverwendet (`crates/ssx3-gfx/src/pack.rs`: `name(&TexKey)`, `crate::tex::{TexKey, Decoded}`), statt sie nachzubauen. Paritäts-Spike zuerst (Risiko laut Spec), dann der Bin, dann das Pinning der Upscale-Modelle.

**Tech Stack:** Rust 2021, bestehende Engine-Crates; keine neuen Dependencies außer wenn ISO/BIG-Lesen fehlt (dann minimal halten).

**Spec:** `C:\GitHub\s2x3\docs\superpowers\specs\2026-11-26-texture-packs-design.md` (Abschnitt „Upstream-PR (Workstream B)")

**Arbeitsort:** Engine-Checkout `~/Projects/ssx3/ssx3-decomp`, Branch `s2x3/dump-textures`. Note in `C:\GitHub\s2x3\docs\upstream\` begleitet den PR (AGENTS.md-Workflow). **Niemals Spieldaten committen** (`orig/`, Dumps).

## Global Constraints

- License-Konventionen des Engine-Repos beachten; Diff klein und isoliert halten (PR gegen `andersfischernielsen/ssx3-decomp`).
- Der Hash-Name MUSS über dieselben Funktionen laufen wie zur Laufzeit: `ssx3_gfx::tex` dekodieren → `TexKey` → `ssx3_gfx::pack::name`. Kein lokales Nachrechnen.
- PNG-Alpha wie im Laufzeit-Dump: Alpha ×2 (0xFF = opak), `pack.rs:100-107`.
- Keine Abhängigkeit von einer bestimmten Disc-Region: das Tool prüft die Seriale wie `tools/setup.py` (SLUS_207.72) und bricht sonst mit lesbarer Meldung ab.

## Review Focus

1. **Parität preswizzelter Formate:** Bei PSM 10/0xc liegt der gehashte Inhalt als Upload-Bild vor, nicht als Datei-Bytes — die Datei-Dekodierung muss exakt das Bild rekonstruieren, das der Upload-Pfad hashen würde. Test in Task 2 (Fixture), Maschinen-Check in Task 4.
2. **TEXA-Default:** Der Laufzeit-Name enthält `texa`; Datei-Texturen ohne explizites TEXA müssen denselben Default-Wert annehmen wie die Engine beim Upload. Test in Task 2.
3. **Doppelte Namen:** Dieselbe Textur unter mehreren Pfaden → nur eine Datei schreiben (Laufzeit-Dump schreibt auch einmalig, `pack.rs:93`). Test in Task 3.
4. **Abbruchbare ISO:** Fehlende/truncate Datei → Fehler mit Pfad und Offset, nicht stilles Überspringen ganzer Archive. Test in Task 3.
5. **Mehrfach-CLUT-Texturen:** Animierter CLUT = ein Name pro CLUT (pack.rs-Doku); der Offline-Walk muss alle CLUT-Varianten erfassen, die das Spiel hochlädt. Prüfpunkt in Task 4 (Differenzzählung).

---

### Task 1: Recherche-Protokoll BIG/SHPS → TexKey

**Files:**
- Create: `docs/notes/dump-textures-parity.md` (Engine-Repo)

**Interfaces:**
- Produces: der dokumentierte Weg von den Datei-Bytes zu `TexKey`-Feldern (`hash`, `format`, `tex0`, `texa`), je PSM: wo die Datei das Feld liefert, wo die Engine einen Default setzt.

- [ ] **Step 1:** Engine-Code lesen (`ssx3-gfx::tex`, Upload-Pfad in `ssx3-engine`/`emu-gs`) und die Herkunft jedes `TexKey`-Felds dokumentieren; offene Punkte als Liste.
- [ ] **Step 2:** Format-Doku gegenlesen (SSX-Modding-Wiki SHPS/BIG; `tools/iso-ls.py` zeigt Disc-Struktur).
- [ ] **Step 3:** Commit: `Document file-to-TexKey mapping`.

### Task 2: Datei-Dekodierung mit Paritäts-Test

**Files:**
- Create: `crates/ssx3-gfx/src/file_tex.rs` (oder passender Ort aus Task 1)
- Test: gleiche Datei, `#[cfg(test)]`

**Interfaces:**
- Produces: `pub fn decode_file_tex(shps_entry: &[u8]) -> anyhow::Result<(TexKey, Decoded)>` — dekodiert eine SHPS-Textur zu Key + RGBA so, wie der Upload-Pfad es hashen würde.

- [ ] **Step 1: Failing tests** — synthetische SHPS-Fixtures bauen (Encoder im Test): 8-bit-CLUT, 16-bit direkt, ein preswizzeltes Format; Assertion: `name(&key)` stabil (Goldwert je Fixture), `Decoded`-Pixel der Fixture entsprechen.
- [ ] **Step 2:** `cargo test -p ssx3-gfx` → FAIL.
- [ ] **Step 3:** Implementieren (Task-1-Protokoll umsetzen; nichts an der Laufzeit-Logik ändern).
- [ ] **Step 4:** PASS. **Step 5:** Commit: `File-side SHPS decode with TexKey parity`.

### Task 3: Bin `ssx3 dump-textures`

**Files:**
- Modify: `crates/ssx3/src/main.rs` (Subcommand-Enum des `ssx3`-Bins)
- Create: `crates/ssx3/src/dump_textures.rs`

**Interfaces:**
- Consumes: `decode_file_tex` (Task 2), ISO-/BIG-Lesen des Workspaces (aus Task 1; falls nur Python vorhanden ist: minimales Rust-BIG-Reader-Modul in `ssx3-gfx`, Diff klein halten).
- Produces: `ssx3 dump-textures --iso <pfad> --out <dir> [--force]` — geht alle BIG-Archive der Disc, schreibt `<out>/<name>.png` (existierende überspringen, außer `--force`), druckt Zusammenfassung (Anzahl Texturen, Archive, übersprungene Fehler je Archivpfad). Exit ≠ 0 mit lesbarer Meldung bei defekten Archiven.

- [ ] **Step 1: Failing tests** — Fixtur-ISO/BIG (aus Task-2-Fixtures): Dateien landen unter erwarteten Namen; doppelter Name nur einmal; defektes Archiv → Fehlermeldung mit Pfad; `--force` überschreibt.
- [ ] **Step 2:** FAIL. **Step 3:** Implementieren. **Step 4:** PASS. **Step 5:** Commit: `Add ssx3 dump-textures subcommand`.

### Task 4: Paritäts-Check gegen den echten Laufzeit-Dump (maschinell)

**Files:**
- Create: `scripts/check-dump-parity.sh` (POSIX; im Engine-Repo erlaubt, enthält keine Spieldaten)

- [ ] **Step 1:** Lokal: Offline-Extraktion der vorhandenen ISO nach `textures/dump-offline/` (auf dem Entwicklerrechner existieren `orig/`, die ISO und 13k Dump-PNGs).
- [ ] **Step 2:** Vergleich: jede Datei in `textures/dump/` muss unter gleichem Namen in `textures/dump-offline/` liegen (Laufzeit ⊆ Offline). Differenzen nach PSM-Klasse in `docs/notes/dump-textures-parity.md` eintragen; Ausschlüsse dokumentieren (Spec-Fallback).
- [ ] **Step 3:** Ergebnis (Zahlen + Ausschlüsse) als Note `C:\GitHub\s2x3\docs\upstream\YYYY-MM-DD-dump-textures.md` mit betroffenem Engine-Commit. Commit: `Record dump parity results`.

### Task 5: Upscale-Tool pinnen und entfestigen

**Files:**
- Modify: `tools/upscale-textures.py`

**Interfaces:**
- Produces: `--in <dir>` / `--out <dir>` (Default wie bisher `textures/dump` → `textures/replace`), Modell-Download von einem **gepinnten Upscayl-Commit** mit optionalem `--require-sha256 <map>`-Check (Default an, Werte im Tool-Header).

- [ ] **Step 1: Failing tests** — Python-Test (`pytest` falls im Repo vorhanden, sonst einfaches `python tools/test_upscale_flags.py`): `--in/--out` werden respektiert; URL-Builder enthält den gepinnten Commit; SHA-Check lehnt abweichende Prüfsumme ab.
- [ ] **Step 2:** FAIL. **Step 3:** Implementieren (kleiner Diff; das Spec-Feld `engine_commit` im Launcher-Manifest liest den Commit aus `git rev-parse HEAD` zur Laufzeit — hier nichts weiter tun).
- [ ] **Step 4:** PASS. **Step 5:** Commit: `Pin upscale toolchain and allow custom in/out dirs`.

### Task 6: PR-Vorbereitung

- [ ] Branch `s2x3/dump-textures` pushen, PR gegen `andersfischernielsen/ssx3-decomp` (Titel: `Add offline texture dump (ssx3 dump-textures)`), Body: Symptom, Repro, Paritätszahlen aus Task 4, Ausschlüsse. Nur diesen Diff — keine Launcher-Arbeit.
- [ ] In `C:\GitHub\s2x3\docs\upstream\`: PR-Link und Merge-Status nachtragen (Gate für „upstream-Bin verfügbar", siehe Launcher-Plan Task 7).

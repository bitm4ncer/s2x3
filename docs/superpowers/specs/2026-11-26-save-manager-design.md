# Save-Manager für den S2X3 Launcher — Kurzdesign (2026-11-26)

## Intent
Mehrere Spielstände (Card-Profile) im Launcher verwalten: anlegen, aktivieren
vor dem Spielstart, löschen, duplizieren, umbenennen — mit Größe, "zuletzt
gespielt" und Saves-Anzeige. Dazu Teilen eines Stands als Link oder kleine
Datei (Messenger-tauglich, ~44 KB gzip).

## Entcheidungen
- **Datei-basiert**: Profile = `saves/cards/<name>.ps2` (vollständige
  Memory-Card-Images). Aktiv = `config.paths.memcard` (neues Feld, Default
  `saves/ssx3-mc0.ps2`); das Spiel bekommt den Pfad via `SSX3_MEMCARD`
  (existiert bereits im Adapter). Kein Kopieren vor dem Start.
- **Teilen**: Export als `.ps2.gz` (gzip, ~44 KB — messenger-tauglich) über
  den Datei-Speichern-Dialog, plus SHA-256-Kürzel (16 Hex-Zeichen) als
  Fingerabdruck zum Vergleichen nach dem Transfer. Import: `.ps2.gz` oder
  rohe `.ps2` über den Datei-Öffnen-Dialog, Superblock wird geprüft. Keine
  Links, kein Server, alles client-side.
- **Kein Career-Fortschritt in v1**: die Card-Struktur gibt Größe, Datum und
  Inhaltsliste her; echten Fortschritt zu parsen ist Engine-Territorium und
  kommt später upstream.

## Nicht-Ziele
- Kein Server, keine Tokens; kein Bearbeiten der Card-Inhalte; kein
  automatischer Cloud-Sync.

## Umsetzung
1. `s2x3-core`: `paths.memcard` Feld (Default → ssx3-mc0.ps2, Legacy-safe).
2. `s2x3-engines/ps2.rs`: `memcard_path()` ehrt `config.paths.memcard`.
3. `s2x3-core`: minimaler Card-FS-Leser (Superblock + Verzeichnisseise) für
   die Saves-Anzeige; bei Parse-Fehler "—".
4. Desktop-Commands: `save_profiles`, `card_new/rename/delete/duplicate/
   activate`, `card_share` (→ Link + .ps2.gz), `card_import` (Link/Datei).
5. UI: Save-Game-Box → Profil-Liste + Aktionen + Share-Zeile; Styles im
   bestehenden Look (flach, abgerundet, halbtransparent).
6. Verification: CLI-Roundtrip (create/activate/share/import), UI-Screenshot.

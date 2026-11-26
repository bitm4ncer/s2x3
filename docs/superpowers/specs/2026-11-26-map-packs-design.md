# Map-Packs im Launcher („Maps"-Feature) — Design

Status: Entwurf, wartet auf Freigabe. Datum: 2026-11-26.

## Intent und Warum

Der Nutzer will SSX-Tricky-Inhalte (die Tricky-Kurse des offiziellen
`tricky`-Plugins und Community-Map-Packs wie ManTheSams Custom-Maps) über den
Launcher spielen: sichtbar, wählbar, per Auto-Download eingerichtet.

Die Engine (ssx3-decomp, upstream) hat dafür bereits alles:

- Plugin = Ordner mit `plugin.toml` (+ `content/`, optional `prepare/`, `src/`).
- Plugin-Peaks/Events erscheinen nativ auf dem Select-Peak-Screen.
- `ssx3 plugins list | prepare <id> | build` als Engine-CLI.
- Welt-Dateien werden per Overlay vor die Disc-Daten gelegt
  (`content/DATA/WORLDS/<world>.big`).
- Direktstart: `SSX3_EVENT=<plugin>/<event>`; An/Aus: `SSX3_PLUGINS=a,b` bzw.
  `SSX3_PLUGINS=0`; Plugin-Ordner: `plugins/` neben den Game-Daten oder
  `SSX3_PLUGIN_DIR`; weitere Disc-Images: `SSX3_DISC_DIR`.

Unsere Lücke ist reine Launcher-Arbeit (ROADMAP Phase 4 „mods/plugins list"):
Katalog mit Auto-Download, Installation, Status, Toggles, Launch-Umgebung.

## Ziele

- „Maps"-Bereich im Desktop-UI und CLI-Parität (`s2x3 maps ...`).
- Katalog (JSON-Index, nur Metadaten) mit **Install-Button = Auto-Download**
  (ZIP laden → Prüfsumme → entpacken nach `paths.plugins` → Status refresh).
- Lokaler Import (ZIP oder Ordner) für alles, was nicht im Katalog ist.
- Pro Plugin: An/Aus (Toggle), Entfernen, Ordner öffnen, Status-Anzeige
  (bereit / braucht Disc / braucht Prepare / braucht Engine-Build).
- `tricky`-Plugin-Flow: Plugin installieren, Tricky-Disc (.iso/.bin) hinterlegen
  (`paths.discs`), `ssx3 plugins prepare tricky` aus dem Launcher starten.
- Launch integriert: `SSX3_PLUGIN_DIR`/`SSX3_DISC_DIR` setzen, deaktivierte
  Plugins über `SSX3_PLUGINS` ausklammern, `ssx3-custom`-Binary bevorzugen,
  wenn ein aktiviertes Plugin Rust-Code hat.
- Optional (erweitert): Direktstart eines Events über `SSX3_EVENT`.

## Nicht-Ziele

- Kein Game-Data im Repo, kein Hosting von Packs: Der Katalog listet nur
  Metadaten/URLs; Inhalte liegen bei den Autoren (z. B. GitHub Releases) bzw.
  entstehen aus der Disc des Nutzers. (Hard Rule 1.)
- Keine automatische Konvertierung von Tricky-ISOs in SSX3-World-Packs (v1).
- Kein Plugin-Authoring-UI, keine Code-Plugin-Verwaltung über den Katalog.
- Kein Netplay, keine Career-/Save-Integration (die Engine hält Plugin-Events
  ohnehin aus der Career raus).
- Xbox-Adapter bleibt ohne Plugins (`capabilities.plugins = false`).

## Anti-Ziele (wäre trotz Funktion ein Fehler)

- Der Launcher dürfte nie stillschweigend Engine-Dateien überschreiben oder
  in den Engine-Checkout schreiben.
- Der Launcher dürfte nie Binärinhalte aus Packs ausführen.
- Kein Auto-Update von Packs ohne sichtbare Aktion (Download immer user-initiiert
  bzw. mit Bestätigung).

## Ansätze (Ingest-Weg — die Frage „Auto-Download-Button?")

1. **Katalog + Ein-Klick-Install + lokaler Import (gewählt).** Der Launcher
   lädt einen JSON-Index (URL konfigurierbar, Standard: Datei im s2x3-Repo bzw.
   Community-Repo), zeigt Einträge als Karten, „Install" lädt das ZIP des
   Autors, prüft SHA-256, entpackt nach `paths.plugins`. Lokaler Import deckt
   alles ab, was nicht im Katalog steht. Trade-off: etwas mehr Aufwand
   (Index-Format, Netzwerk-Code, Sicherheits-Checks) — aber genau das vom
   Nutzer gewünschte „Auto-Download-Button"-Erlebnis.
2. **Nur lokaler Import.** Deutlich weniger Code, kein Netzwerk. Trade-off:
   Für jeden Pack manuell downloaden und durchs Datei-Dialog-Menü — genau das,
   was der Nutzer nicht will.
3. **Voller „Store"** (Browsing einzelner Maps, Bewertungen, Auto-Updates im
   Hintergrund). Trade-off: Es gibt heute noch keine gefüllte Katalog-Infra-
   struktur und keine Metadata-Quelle dafür; Aufwand ohne Daten. Kann später
   auf Ansatz 1 aufgesetzt werden, indem der Index reichere Felder bekommt.

Entscheidung: Ansatz 1. Ansatz 3 bleibt offen, der Index wird so entworfen,
dass er später erweitert werden kann.

## Entscheide (so wird es gebaut)

- **Plugin-Ordner:** neu `paths.plugins` (Default `<Config-Basis>/plugins`,
  analog `paths.saves`/`paths.texreplace` in `Config::resolved`) und
  `paths.discs` (Default `<Config-Basis>/discs`). Der Launcher setzt bei
  Launch und Prepare `SSX3_PLUGIN_DIR` bzw. `SSX3_DISC_DIR`. Es wird nie in
  den Engine-Checkout geschrieben.
- **Katalog-Index:** JSON-Datei, Felder pro Eintrag: `id`, `name`, `author`,
  `description`, `version`, `url` (ZIP), optional `sha256`, `size`,
  `plugin_id` (id aus plugin.toml, für Update-Erkennung), `peaks`/`events`
  (Vorschau), `source_note`. Mehrere Quellen möglich (Liste von URLs,
  Standard-Quelle im Config-Default, im UI editierbar).
  **Standard-Quelle:** eine gepinnte Raw-URL in diesem Repo
  (`catalog/maps.json`, nur Metadaten — Hard-Rule-konform). Im Launcher liegt
  eine eingebundene Fallback-Kopie; ist die Quelle nicht erreichbar, zeigt das
  UI die zuletzt erfolgreich geladene Liste (Cache) mit Hinweis, nie einen
  leeren Fehler ohne Erklärung.
- **Install-Überschreibung:** Ein Pack wird immer erst in einen Temp-Ordner
  entpackt und validiert (`plugin.toml` parsebar), dann atomar getauscht
  (alten Ordner beiseite verschieben, neuen einsetzen, alten löschen; bei
  Fehler zurück). Katalog-Install mit bestehender `plugin_id` = Update
  (ersetzen, Versions-Hinweis im UI). Lokaler Import über eine bestehende id
  fragt vorher nach (niemals stillschweigend überschreiben).
- **Pack-Format:** ZIP mit genau einem Plugin-Ordner im Root (enthält
  `plugin.toml`, …). Beim Entpacken: Zip-Slip-Schutz (keine `..`/absoluten
  Pfade, nichts außerhalb des Zielordners), Größe begrenzen, `plugin.toml`
  muss parsebar sein (TOML), sonst Fehler mit lesbarer Meldung.
- **Status pro Plugin:** Der Launcher liest `plugin.toml` selbst (id, api,
  Peaks/Events-Namen, Vorhandensein von `[inputs]`/`[prepare]`/`[code]`,
  `content/DATA/WORLDS/*.big`). Zusätzlich läuft `ssx3 plugins list` (Engine-
  Binary) mit gesetzten Env-Variablen und dessen Ausgabe wird im Detail-
  Bereich roh angezeigt (single source of truth für „was fehlt"). Maschinen-
  lesbares `--json` ist ein upstream-Follow-up (kleiner PR, eigener
  Workstream-B-Eintrag in `docs/upstream/`); bis dahin parsen wir nur
  unseren eigenen TOML-Read und zeigen die Engine-Ausgabe als Text.
- **Prepare:** Button pro Plugin mit `[prepare]`: startet
  `ssx3 plugins prepare <id>` im Engine-Root mit `SSX3_ISO`,
  `SSX3_PLUGIN_DIR`, `SSX3_DISC_DIR` — Ausgabe läuft in ein Log
  (Muster existiert schon: `spawn_build` im Tauri-Backend).
- **Code-Plugins:** sichtbar mit Badge „braucht Engine-Build". Erweiterter
  Button „Plugins bauen" führt `cargo run --profile game -p ssx3 --bin ssx3
  -- plugins build` im Engine-Root aus (nutzer-initiiert, Log wie oben).
  Launch bevorzugt `target/ssx3-custom/ssx3-custom`, wenn vorhanden und ein
  aktiviertes Plugin `[code]` hat, sonst wie gehabt `target/game|dist/ssx3`.
- **Toggle:** Pro Plugin enabled/disabled (persistiert in der Config,
  `plugins: BTreeMap<String, bool>`). Launch: wenn alle (installierten)
  enabled → Variable weglassen (Engine-Default: alle laden); sonst
  `SSX3_PLUGINS=<enabled-ids>`. Der Launcher liest dafür auch die Plugin-Ids
  aus dem `plugins/`-Ordner des Engine-Checkouts (nicht verwaltet, nur
  gelesen) und nimmt sie immer in die Enable-Liste auf; eigene Plugins in
  `SSX3_PLUGIN_DIR` tauchen zusätzlich als read-only Einträge im UI auf. Sind
  alle verwalteten Plugins disabled → `SSX3_PLUGINS=0`. So werden fremd
  installierte Plugins nicht stillschweigend ausgeklammert.
- **Direktstart (erweitert):** Freitext-Feld „Event direkt starten"
  (`<plugin>/<event>`), landet als `SSX3_EVENT` in der Launch-Umgebung.
- **CLI:** `s2x3 maps list` (Status-Tabelle), `maps install <id|url|pfad>`,
  `maps remove <plugin-id>`, `maps enable|disable <plugin-id>`,
  `maps refresh` (Katalog neu laden), `maps prepare <plugin-id>`.
  Shared Logik liegt in `s2x3-core` (neues Modul `plugins`/`maps`), damit CLI
  und Tauri dieselben Funktionen nutzen (Muster: „Everything the future UI
  needs is here first").
- **Tauri:** neues Modul `apps/desktop/src-tauri/src/maps.rs` (Muster
  `cards.rs`: `#[tauri::command]`-Funktionen + Dto-Typen). Frontend: neuer
  „Maps"-Abschnitt in `apps/desktop/src/index.html` + Handler in `main.js`
  (+ `style.css` im SSX-3-Look: navy/blau/orange, angewinkelte Panels).
- **Katalog-Inhalte zum Start:** Index-Datei im Repo (nur Metadaten!). Der
  offizielle `tricky`-Plugin-Code stammt aus der Upstream-Arbeit
  (`npiriou/ssx3-decomp-more`); ein fertiges ZIP-Artefakt gibt es heute noch
  nicht. Bis dahin ist der tricky-Eintrag im Katalog eine **Link-Karte**
  (öffnet die Upstream-Anleitung), installierbar über den lokalen Import;
  sobald upstream ein ZIP publiziert, wandert es in den Katalog (Workstream-B
  Eintrag in `docs/upstream/`). Abnahme-Schritt 3 ist damit an dieses
  Artefakt gebunden; bis dahin wird der Prepare-Flow mit dem
  `menu-test`-Beispiel der Engine getestet. ManTheSams Maps erscheinen im
  Katalog erst, wenn es sie als Pack/ZIP gibt (heute: modded Tricky-ISOs);
  der lokale Import nimmt jedes gültige Plugin-Paket.
- **Sequenzierung:** Parallel laufende Save-Manager-Arbeiten (`cards.rs`,
  eigenes Design) berühren dieselbe Dateien (`Config`, `lib.rs` invoke
  handler, `index.html`, `main.js`, `style.css`). Der Maps-Plan vergibt
  eigene Abschnitts-IDs/Command-Namen (`maps_*`, „Maps"-Panel), damit beide
  Features unabhängig landen können.

## Sicherheits-Regeln

- Nur https-Katalog-/-Pack-URLs. SHA-256, wenn im Index angegeben, wird
  geprüft; Abweichung = Fehler mit lesbarer Meldung.
- Zip-Slip-Schutz beim Entpacken; keine Dateien außerhalb des Plugin-Ordners.
- Packs werden nie ausgeführt; sie sind Daten (`plugin.toml` + Content).
- `cargo test`/`cargo build` Workspace + `cargo build` in
  `apps/desktop/src-tauri` müssen grün bleiben.

## Abnahme (aus Nutzersicht)

1. „Maps"-Tab öffnen → Katalog lädt, Einträge zeigen Name/Autor/Peaks/Größe.
2. „Install" → Fortschritt → Plugin erscheint unter „Installiert" mit Status.
3. Tricky-Plugin installieren → Status „braucht Tricky-Disc (SLUS-20326)" →
   Tricky-ISO nach `discs/` legen (oder Dialog) → „Prepare" → Status „bereit".
4. Spiel starten → Select-Peak-Screen listet „Peak Tricky" bzw. Plugin-Peaks
   unter den drei echten Peaks → Event starten und fahren.
5. Toggle aus → Neustart → Peak ist weg; Toggle an → wieder da.
6. Plugin entfernen → Ordner ist weg, Config sauber.

## Dem Builder überlassen

- Datei-/Funktionsaufteilung im Detail (neues `s2x3-core`-Modul, CLI-Sub-
  command, `maps.rs`, Frontend-Markup).
- exakte Fehler- und Log-Formulierungen (lesbar, wie im Repo üblich),
- TOML-Teil-Parser (oder kleines Crate) für `plugin.toml`,
- Tests (Unit für Index-Parse/Zip-Extrakt/Toggle-Logik; Smoke mit
  `examples/plugins/menu-test` der Engine, falls ein Checkout vorhanden ist).

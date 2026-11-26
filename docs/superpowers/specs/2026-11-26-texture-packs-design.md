# Texture-Packs im Launcher („Textures"-Feature) — Design

Status: Entwurf, wartet auf Freigabe. Datum: 2026-11-26.

## Intent und Warum

Der Nutzer will die Spielgrafik im Launcher verwalten können, ohne je einen
technischen Flow mitzuerleben:

- **Xbox-Buttons** (Texture-Patch statt PlayStation-2-Glyphen) sollen direkt
  funktionieren, sobald die Disc hinterlegt ist — ohne Dump-Lauf, ohne
  Menüs durchklicken, ohne Rennen fahren.
- **HD-Texturen** (KI-Upscale der Original-Texturen) sollen entstehen, ohne
  dass das Spiel dafür gestartet wird. Der sonst übliche Weg (Spiel starten,
  alles abklappern, damit `SSX3_TEX_DUMP` die Texturen sammelt) ist zu
  umständlich und soll vom Launcher verdeckt werden.
- Ein eigener **„Textures"-Tab** im Desktop-UI bündelt das: Packs
  hochskalieren, auswählen und einzelne Texturen manuell austauschen.

Die Engine (ssx3-decomp, upstream) hat dafür bereits alles Laufzeit-Seitige
(`crates/ssx3-gfx/src/pack.rs`):

- **Name einer Textur** = FNV-1a-64 (`0xcbf29ce484222325` /
  `0x100000001b3`) über `TexKey { hash(pixel+CLUT), format, TEX0, TEXA }`,
  als 16 Hex-Ziffern. Der Name hängt nur vom Dekodier-Ergebnis und den
  GS-Parametern ab — **nicht davon, ob das Spiel oder ein Tool dekodiert**.
- `SSX3_TEX_DUMP=1|<dir>`: jede dekodierte Textur einmal als
  `<dir>/<name>.png` (Alpha ×2, 0xFF = opak).
- `SSX3_TEX_REPLACE=<dir>` (bzw. der Standardpfad
  `textures/replace`, `0` = aus): PNG unter dem Namen ersetzt die Textur,
  **jede Größe** (Seitenverhältnis beibehalten, UVs sind normalisiert),
  volles Mip-Ketten werden beim Laden erzeugt.

Unsere Lücken sind: (1) die **Offline-Extraktion** — ISO direkt lesen und
dieselben Hash-Namen erzeugen, ohne Spielstart (upstream-Tool); (2) reine
Launcher-Arbeit — Packs verwalten, Hintergrund-Job, „Textures"-Tab.

## Ziele

- Upstream-Tool **`ssx3 dump-textures`**: liest die ISO, dekodiert alle
  statischen Texturen (BIG-Archive, SHPS) mit **denselben Engine-Funktionen**
  (`ssx3-gfx::tex` dekodieren, `ssx3-gfx::pack::name` benennen) in PNGs mit
  Laufzeit-Dump-Namen. Kein Spielstart, kein GPU-Bedarf, Laufzeit
  Sekunden bis wenige Minuten.
- **Xbox-Prompt-Pack v1**: eigen gezeichnete A/B/X/Y-Glyphe unter den
  Hash-Namen der PS2-Glyphen (Hashes werden einmalig Entwickler-seitig aus
  dem laufenden Dump ermittelt; Hash-Namen sind bloß Zahlen, kein
  EA-Material). Pack ist Own-Art und damit frei verteilbar; im Katalog und
  per lokalem Import. Funktionsfähig **ohne jede Extraktion** auf
  Nutzerseite.
- **Automatische Hintergrund-Extraktion**: sobald die hinterlegte ISO neu
  oder erstmals bekannt ist, läuft `ssx3 dump-textures` einmal dagegen — im
  Hintergrund, nur eine dezente Statuszeile im UI. Kein Spielstart, kein
  Fenster. (Trigger-Details unter „Entscheide".)
- **„Textures"-Tab** (Desktop-UI) und CLI-Parität (`s2x3 textures ...`):
  - Karten je Pack: Vorschau, Name, Autor, Art (own-art / upscale), Status.
  - Aktionen: An/Aus-Toggle, Ordner öffnen, Entfernen, lokaler Import
    (ZIP oder Ordner).
  - **„HD hochskalieren"**: aus dem lokalen Dump ein ESRGAN-Pack erzeugen
    (`tools/upscale-textures.py` der Engine, gepinnt, siehe unten) — als
    sichtbarer Job mit Fortschritt, user-initiiert.
  - **Manueller Tausch**: Ordner öffnen → einzelne `<hash>.png` ersetzen;
    nach Neustart aktiv (Engine lädt beim Start).
- **Launch-Integration**: `SSX3_TEX_REPLACE` zeigt auf einen
  Launcher-verwalteten Merge-Ordner der aktivierten Packs.

## Nicht-Ziele

- Kein Auto-Play-Szenario (Ansatz „Spiel klickt sich selbst durch" ist
  verworfen: fragil, schwergewichtig, kein Vorteil gegenüber der
  Offline-Extraktion).
- Kein Game-Data im Repo, kein Hosting von Packs: Der Katalog listet nur
  Metadaten/URLs; **Upscale-Packs (aus Dump-Daten) entstehen ausschließlich
  lokal aus der Disc des Nutzers** und werden nie verbreitet. Own-Art-Packs
  dürfen im Katalog stehen. (Hard Rule 1.)
- Kein Texture-Editor im Launcher — manueller Tausch passiert im Dateisystem
  (Ordner öffnen), nicht in eigenem UI.
- Kein Live-Reload: Änderungen greifen beim nächsten Spielstart.
- Xbox-Adapter: Pack-Verwaltung funktioniert, sofern der Adapter
  Texture-Replacement unterstützt (`capabilities`); sonst dezenter Hinweis.

## Ansätze (Extraktions-Weg)

1. **Offline-Extraktor im Engine-Workspace (gewählt).** Neues Bin
   `ssx3 dump-textures` neben der Engine, das ISO → BIG → SHPS dekodiert und
   dabei `ssx3-gfx::tex`/`pack::name` **direkt importiert** — Hash-Parität
   ist damit Konstruktion, kein Nachbau. Launcher läuft es im Hintergrund
   bei ISO-Import. Trade-off: upstream-PR nötig (eigener Workstream-B-Note);
   dafür unsichtbar, schnell, deterministisch.
2. **Spiel headless durchklicken** (Auto-Input durch Menüs, Dump an).
   Kein upstream-PR, aber: Input-Skripte gegen die Menü-Statemachine, Timing,
   sichtbares Fenster minimiert, Minuten statt Sekunden, und Rennen müssten
   für Laufzeit-Effekte mitgefahren werden. Gegen den Intent
   („User merkt nichts").
3. **Nur Dev-seitiger Dump, Nutzer macht nichts.** Reicht für das
   Xbox-Pack (einmalige Hash-Ermittlung), aber nicht für den HD-Fall —
   der Nutzer soll selbst upscalbaren Inhalt aus **seiner** Disc bekommen
   (rechtlich zwingend lokal).

Entscheidung: Ansatz 1, plus das vorgefertigte Xbox-Pack (aus Ansatz 3)
damit die Buttons sofort funktionieren.

## Entscheide (so wird es gebaut)

- **Pfade:** neu `paths.textures` (Default `<Config-Basis>/textures`) mit
  Unterordnern `dump/` (Extraktions-Ergebnis), `packs/<id>/` (installierte
  Packs), `active/` (Merge-Ordner der aktivierten Packs). Der Launcher
  setzt bei Launch `SSX3_TEX_REPLACE=<...>/active`. Es wird nie in den
  Engine-Checkout geschrieben (Muster: `paths.plugins` aus dem Maps-Design).
- **Hash-Parität zuerst (Risiko-Puffer):** Der upstream-PR beginnt mit einem
  Spike, der die Abbildung SHPS/BIG-Datei → Laufzeit-`TexKey` belegt —
  Datei-Dekodierung muss dieselben Felder liefern wie der Upload-Pfad
  (inkl. TEXA-Default für Datei-Texturen, formatierte/preswizzelte PSMs).
  Hält die Parität für eine Texturklasse nicht: dokumentierte Ausschlüsse
  im Manifest, und der Laufzeit-Dump bleibt als „Erweitert"-Weg im
  „Textures"-Tab (geführter Menü-Spaziergang, sichtbar — nicht versteckt).
- **Hintergrund-Extraktion:** Trigger ist die in der Config hinterlegte ISO
  (`paths.iso`): erster bekannter Wert oder Wechsel des ISO-Hashes (Merker
  in der Config). Die ISO wird **in place referenziert**, nie kopiert
  (mehrere GB). Kein eigenes Disc-Import-Feature — das bleibt Phase 2 bzw.
  Maps (`paths.discs`); landet später eine neue ISO dort, zieht die
  Extraktion ebenfalls an. UI zeigt nur eine Statuszeile („Texturen werden
  gelesen …/fertig/gescheitert" mit lesbarem Fehler und „Erneut versuchen").
- **Upscale-Job:** Button im „Textures"-Tab, führt
  `tools/upscale-textures.py` (Engine-Checkout) mit gepinnter
  Real-ESRGAN-Version aus; die Modelle werden auf einen gepinnten
  Upscayl-Commit mit SHA-256-Prüfung geladen (heute zieht das Tool von
  `main` — wird im upstream-PR mitgepinnt). **Vorbedingungen prüft der
  Launcher vorab mit lesbarer Meldung** (Python ist Engine-Voraussetzung,
  Pillow und eine Vulkan-fähige GPU nicht): Wer sie nicht erfüllt, sieht
  vor dem Klick, was fehlt — kein toter Button. Toolchain und Modelle
  landen über `--esrgan`/Ausgabepfade unter `paths.textures/.tools`, nie im
  Engine-Checkout. Ausgabe: Pack in `packs/<name>/`, automatisch aktiviert.
  Manifest hält Herkunft fest (Tool-Version, Modell, Scale) —
  Reproduzierbarkeit, keine Cross-GPU-Bit-Identität versprochen.
- **Merge-Logik:** `active/` enthält die Dateien aller aktivierten Packs.
  Gleicher Hash in zwei aktiven Packs → Priorität = Reihenfolge im UI
  (Drag/„oben"-Steuerung v2; v1: erstes aktiviertes Pack gewinnt, Konflikt
  wird im UI mit Warnhinweis gezeigt). Merge wird bei jedem Toggle neu
  gebaut (Dateien kopieren, atomar tauschen).
- **Pack-Format:** Ordner mit `manifest.toml` (`name`, `author`, `version`,
  `kind` = `own-art` | `upscale`, optional `targets`: `hash → beschriftung`)
  + `textures/<hash>.png` (Alpha-Skala wie die Engine: PNG-Wert ×2 = opak;
  Seitenverhältnis der Originaltextur beibehalten). ZIP-Import mit
  Zip-Slip-Schutz, Größenlimit, TOML-Validierung (Muster: Maps-Design).
- **Xbox-Pack:** Own-Art-Glyphe (eigene Zeichnung, Repo-Lizenz) unter den
  Hash-Namen der PS2-Glyphen; `targets` dokumentiert, welcher Screen welche
  Glyphe zeigt. Die Glyphe-Assets entstehen in der laufenden Xbox-Buttons-
  Arbeit (paralleler Agent) — dieses Feature behandelt sie als
  Eingangs-Abhängigkeit. **v1 legt das Pack dem Launcher bei** (own-art,
  Repo-Lizenz — verteilbar), damit Abnahme 2 ohne Hosting testbar ist;
  dazu Katalog-Bootstrap `catalog/textures.json` (nur Metadaten). Lokale
  Importe mit fremden Hashes sind erlaubt (Nutzer bringt eigene Packs
  mit), aber immer nur als Daten behandelt.
- **Upstream-PR (Workstream B):** `ssx3 dump-textures` Bin im
  Engine-Workspace; kleiner, isolierter Diff; Note in `docs/upstream/`
  (Symptom: Offline-Extraktion fehlt; Akzeptanz: Hash-Parität — der
  Laufzeit-Dump einer Menü-Session muss eine Teilmenge der
  Offline-Extraktion sein; UI-Hashes müssen deckungsgleich sein).
- **Ablösung des Phase-3-Mechanismus:** Der alte Weg
  (`ui.buttons == "xbox"` + `paths.texreplace`) wird vom Pack-System
  ersetzt — kein paralleles zweites Verhalten. Migration: ein bestehender
  `paths.texreplace`-Ordner wandert einmalig als Pack nach
  `packs/texreplace-import/` (An/Aus wie jedes Pack). Der Launcher setzt
  `SSX3_TEX_REPLACE` **immer** explizit: `active/`, wenn Pack(s) aktiv,
  sonst `0` — damit ist „Toggle aus → Original-Look" garantiert, unabhängig
  vom Engine-Default-Ordner.
- **Versions-Staleness:** Manifests und der ISO-Merker halten den
  Engine-Commit fest, der sie erzeugt hat; ändert sich der Engine-Stand
  (Update), markiert der Launcher Dump und Packs als „veraltet — neu
  erzeugen?" statt sie still weiterlaufen zu lassen (Feld ab Tag 1, nicht
  nachrüstbar).
- **CLI:** `s2x3 textures list` (Packs + Status), `textures import
  <pfad>`, `textures remove <id>`, `textures enable|disable <id>`,
  `textures extract` (Extraktion nochmal laufen lassen),
  `textures upscale` (ESRGAN-Job). Shared Logik in `s2x3-core` (neues
  Modul `textures`), damit CLI und Tauri dieselben Funktionen nutzen.
- **Tauri:** neues Modul `apps/desktop/src-tauri/src/textures.rs` (Muster
  `cards.rs`/geplanter `maps.rs`: `#[tauri::command]`-Funktionen +
  Dto-Typen). Frontend: neuer „Textures"-Abschnitt in
  `apps/desktop/src/index.html` + Handler in `main.js` (+ `style.css` im
  SSX-3-Look). Eigene Abschnitts-IDs/Command-Namen (`textures_*`), damit
  Maps/Saves/Textures unabhängig landen können.
- **Vorschau im Tab:** Pack-Karte zeigt 1–2 Beispiel-PNGs aus dem Pack
  (keine Game-Screenshots nötig); das Xbox-Pack bringt eigene Vorschauen
  mit. Später: Launcher-Screenshot vom laufenden Spiel (v2).

## Sicherheits-Regeln

- Nur https-URLs; gepinnte Downloads mit SHA-256-Prüfung (Real-ESRGAN,
  Modelle). Abweichung = Fehler mit lesbarer Meldung.
- Zip-Slip-Schutz beim Entpacken; keine Dateien außerhalb des Pack-Ordners;
  Packs werden nie ausgeführt — sie sind Daten.
- Kein Spieldatum im Repo oder in Releases: Dump- und Upscale-Ausgaben
  bleiben unter `paths.textures` (lokale Nutzerdaten).
- `cargo test`/`cargo build` Workspace + `cargo build` in
  `apps/desktop/src-tauri` müssen grün bleiben.

## Abnahme (aus Nutzersicht)

1. Disc/ISO hinterlegen → Statuszeile „Texturen werden gelesen" verschwindet
   von selbst; Spiel ist währenddessen normal startbar.
2. „Textures"-Tab öffnen → „Xbox-Buttons"-Pack aktivieren → Spiel starten →
   Menüs zeigen A/B/X/Y. Kein weiterer Schritt.
3. „HD hochskalieren" → Fortschrittsanzeige → Pack erscheint, aktiviert →
   Spielstart → Texturen sichtbar schärfer.
4. „Ordner öffnen" → einzelne `<hash>.png` durch eigene ersetzen →
   Spielstart → die eine Textur ist getauscht.
5. Toggle aus → Spielstart → Original-Look. Entfernen → Ordner und Eintrag
   sind weg, Config sauber.

## Dem Builder überlassen

- Datei-/Funktionsaufteilung im Detail (neues `s2x3-core`-Modul,
  CLI-Subcommand, `textures.rs`, Frontend-Markup).
- exakte Fehler- und Log-Formulierungen (lesbar, wie im Repo üblich),
- TOML-Teil-Parser für `manifest.toml` (Muster: `plugin.toml` im Maps-Plan),
- Merge-Implementierung (kopieren vs. Hardlinks — Builder-Entscheid mit
  Blick auf Windows/Linux),
- Tests (Unit: Manifest-Parse, Zip-Extrakt, Merge/Priorität; Smoke:
  Offline-Extraktion gegen eine Fixtur-ISO wenn vorhanden; Hash-Paritäts-
  Test gehört in den upstream-PR, nicht hierher).

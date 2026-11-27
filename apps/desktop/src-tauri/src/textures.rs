//! Texture pack commands for the desktop UI: list/import/toggle/remove,
//! the background extraction against the ISO, the upscale job, and the
//! generated `xbox-buttons` pack (spec: textures design).
//!
//! Packs live under `paths.textures`; the engine gets the merge folder
//! `active/` as `SSX3_TEX_REPLACE` — always explicit, `0` when nothing is
//! enabled.

use s2x3_core::textures::{self as tx, MergeReport, PackEntry, MANIFEST};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::Emitter;

/// One pack as the UI shows it.
#[derive(Debug, Clone, Serialize)]
pub struct TexturePackDto {
    pub id: String,
    pub name: String,
    pub author: String,
    pub version: String,
    pub kind: String,
    pub texture_count: usize,
    pub enabled: bool,
    pub error: Option<String>,
    pub conflicts: usize,
    /// 1–2 example images from the pack's `preview/` folder, if any.
    pub previews: Vec<String>,
}

fn dto_from(e: &PackEntry, enabled: Option<bool>, conflicts: usize) -> TexturePackDto {
    let (name, author, version, kind, previews) = match &e.pack {
        Some(p) => (
            p.manifest.name.clone(),
            p.manifest.author.clone(),
            p.manifest.version.clone(),
            p.manifest.kind.to_string(),
            preview_list(&p.dir),
        ),
        None => (
            e.id.clone(),
            String::new(),
            String::new(),
            "?".into(),
            Vec::new(),
        ),
    };
    TexturePackDto {
        id: e.id.clone(),
        name,
        author,
        version,
        kind,
        texture_count: e.texture_count,
        enabled: enabled.unwrap_or(false),
        error: e.error.clone(),
        conflicts,
        previews,
    }
}

fn preview_list(pack_dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(pack_dir.join("preview")) {
        for f in rd.flatten() {
            if f.path()
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("png"))
            {
                out.push(f.path().display().to_string());
                if out.len() == 2 {
                    break;
                }
            }
        }
    }
    out
}

/// Write the `xbox-buttons` pack manifest once; never overwrite.
fn write_manifest_if_missing(dir: &Path) {
    let path = dir.join(MANIFEST);
    if path.is_file() {
        return;
    }
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(
        path,
        "name = \"Xbox Buttons\"\n\
         author = \"s2x3\"\n\
         version = \"1.0.0\"\n\
         kind = \"upscale\"\n\
         [targets]\n\
         \"f897162c0f661c8b\" = \"Steuerkreuz-Atlas (Frontend)\"\n\
         \"75df99fccd7d9d0b\" = \"Steuerkreuz-Atlas (Pause/HUD)\"\n",
    );
}

/// Generate the `xbox-buttons` pack from the texture dump: patch the two
/// known atlases with the launcher's own glyphs (no game art in the repo —
/// the input atlas stays in the dump, the output stays on this machine),
/// manifest it, enable it while the user wants Xbox prompts, merge.
pub fn generate_xbox_pack(config: &mut s2x3_core::Config) -> anyhow::Result<()> {
    let dir = config.paths.textures_packs().join("xbox-buttons");
    let out = dir.join("textures");
    let dump = config.paths.textures_dump();
    if !dump.is_dir() {
        return Ok(());
    }
    crate::texreplace::patch(&dump, &out)?;
    write_manifest_if_missing(&dir);
    if config.ui.buttons == "xbox" {
        config
            .textures
            .enabled
            .entry("xbox-buttons".into())
            .or_insert(true);
    }
    config.textures.generated_xbox = true;
    merge_active(config)?;
    config.save()?;
    Ok(())
}

/// Rebuild `active/` from the config state; logs conflicts to stderr.
fn merge_active(config: &s2x3_core::Config) -> anyhow::Result<MergeReport> {
    let report = tx::merge(
        &config.paths.textures_packs(),
        &config.paths.textures_active(),
        &config.textures.enabled,
        &config.textures.priority,
    )?;
    if !report.conflicts.is_empty() {
        eprintln!(
            "texture packs: several active packs replace the same textures: {}",
            report.conflicts.join(", ")
        );
    }
    Ok(report)
}

fn pack_dtos(config: &s2x3_core::Config) -> Vec<TexturePackDto> {
    let conflicts = tx::conflicts(
        &config.paths.textures_packs(),
        &config.textures.enabled,
        &config.textures.priority,
    );
    tx::list(&config.paths.textures_packs())
        .iter()
        .map(|e| {
            let enabled = config.textures.enabled.get(&e.id).copied();
            let conflict_count = conflict_hits(e, &conflicts);
            dto_from(e, enabled, conflict_count)
        })
        .collect()
}

/// How many of the pack's texture files are claimed by another enabled pack
/// too — counted on the actual file stems, so it works for packs whose
/// manifest lists no targets.
fn conflict_hits(e: &PackEntry, conflicts: &[String]) -> usize {
    let Some(p) = &e.pack else {
        return 0;
    };
    std::fs::read_dir(p.dir.join("textures"))
        .map(|rd| {
            rd.flatten()
                .filter_map(|f| {
                    let stem = f.path().file_stem()?.to_str()?.to_ascii_lowercase();
                    conflicts.iter().any(|h| h == &stem).then_some(())
                })
                .count()
        })
        .unwrap_or(0)
}

/// Open a file dialog and import the picked pack (zip or folder).
#[tauri::command]
pub async fn textures_pick_import() -> Result<Vec<TexturePackDto>, String> {
    let picked = rfd::AsyncFileDialog::new()
        .add_filter("Texture pack", &["zip"])
        .pick_file()
        .await
        .ok_or_else(|| "cancelled".to_string())?;
    textures_import(picked.path().display().to_string())
}

/// Open a folder dialog and import the picked folder as a pack.
#[tauri::command]
pub async fn textures_pick_import_folder() -> Result<Vec<TexturePackDto>, String> {
    let picked = rfd::AsyncFileDialog::new()
        .pick_folder()
        .await
        .ok_or_else(|| "cancelled".to_string())?;
    textures_import(picked.path().display().to_string())
}

#[tauri::command]
pub fn textures_list() -> Result<Vec<TexturePackDto>, String> {
    let (config, _) = crate::load().map_err(|e| format!("{e:#}"))?;
    Ok(pack_dtos(&config))
}

#[tauri::command]
pub fn textures_import(path: String) -> Result<Vec<TexturePackDto>, String> {
    let (config, _) = crate::load().map_err(|e| format!("{e:#}"))?;
    tx::import(Path::new(&path), &config.paths.textures_packs(), false)
        .map_err(|e| format!("{e:#}"))?;
    merge_active(&config).map_err(|e| format!("{e:#}"))?;
    config.save().map_err(|e| format!("{e:#}"))?;
    Ok(pack_dtos(&config))
}

#[tauri::command]
pub fn textures_remove(id: String) -> Result<Vec<TexturePackDto>, String> {
    let (mut config, _) = crate::load().map_err(|e| format!("{e:#}"))?;
    tx::remove(&config.paths.textures_packs(), &id).map_err(|e| format!("{e:#}"))?;
    config.textures.enabled.remove(&id);
    config.textures.priority.retain(|p| p != &id);
    merge_active(&config).map_err(|e| format!("{e:#}"))?;
    config.save().map_err(|e| format!("{e:#}"))?;
    Ok(pack_dtos(&config))
}

#[tauri::command]
pub fn textures_toggle(id: String, enabled: bool) -> Result<Vec<TexturePackDto>, String> {
    let (mut config, _) = crate::load().map_err(|e| format!("{e:#}"))?;
    config.textures.enabled.insert(id.clone(), enabled);
    merge_active(&config).map_err(|e| format!("{e:#}"))?;
    config.save().map_err(|e| format!("{e:#}"))?;
    Ok(pack_dtos(&config))
}

/// Background job marker so a second call does not start a second run.
static EXTRACT_RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Run the offline extraction in the background: on ISO change, read every
/// texture from the disc into `paths.textures/dump` (no game start), then
/// (re)generate the xbox-buttons pack. Emits `textures-extract` events.
#[tauri::command]
pub fn textures_extract(app: tauri::AppHandle) -> Result<String, String> {
    use std::sync::atomic::Ordering;
    if EXTRACT_RUNNING.swap(true, Ordering::SeqCst) {
        return Ok("already running".into());
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let emit = |state: &str, message: &str| {
            let _ = app.emit(
                "textures-extract",
                serde_json::json!({ "state": state, "message": message }),
            );
        };
        emit("running", "Reading textures from the disc …");
        let r = (|| -> anyhow::Result<()> {
            let (mut config, _) = crate::load()?;
            if !config.paths.iso.is_file() {
                anyhow::bail!(
                    "disc image not found: {} — set the disc image first",
                    config.paths.iso.display()
                );
            }
            let current = tx::iso_marker(&config)?;
            // The extractor is idempotent (existing files are skipped), so an
            // explicit click always runs it: the pack follows the dump.
            let summary = tx::run_extract(&config)?;
            config.textures.iso_marker = current;
            generate_xbox_pack(&mut config)?;
            config.save()?;
            emit("done", &summary);
            Ok(())
        })();
        match r {
            Ok(()) => {}
            Err(e) => emit("error", &format!("{e:#}")),
        }
        EXTRACT_RUNNING.store(false, Ordering::SeqCst);
    });
    Ok("started".into())
}

/// Run the ESRGAN upscale in the background; emits `textures-upscale`.
#[tauri::command]
pub fn textures_upscale(app: tauri::AppHandle) -> Result<String, String> {
    static RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    use std::sync::atomic::Ordering;
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Ok("already running".into());
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let emit = |state: &str, message: &str| {
            let _ = app.emit(
                "textures-upscale",
                serde_json::json!({ "state": state, "message": message }),
            );
        };
        emit("running", "Upscaling textures (this takes minutes) …");
        let r = (|| -> anyhow::Result<String> {
            let (mut config, _) = crate::load()?;
            let mut on_line = |line: &str| emit("progress", line);
            let pack = tx::run_upscale_streaming(&config, &mut on_line)?;
            // Enable BEFORE merging: the merge only copies enabled packs, so
            // merging first would ship an active set without the HD files.
            tx::enable_and_merge(&mut config, "hd-upscale")?;
            config.save()?;
            Ok(pack.display().to_string())
        })();
        match r {
            Ok(p) => emit("done", &p),
            Err(e) => emit("error", &format!("{e:#}")),
        }
        RUNNING.store(false, Ordering::SeqCst);
    });
    Ok("started".into())
}

/// Open a pack's folder (or the dump when the id is empty) in the system
/// file manager.
/// Whether re-reading the disc makes sense: the ISO changed, or the dump
/// grew since the last pack patch.
#[tauri::command]
pub fn textures_dump_stale() -> Result<bool, String> {
    let (config, _) = crate::load().map_err(|e| format!("{e:#}"))?;
    let iso_moved = tx::iso_marker(&config)
        .map(|m| m != config.textures.iso_marker)
        .unwrap_or(true);
    let out = config
        .paths
        .textures_packs()
        .join("xbox-buttons")
        .join("textures");
    Ok(iso_moved || !crate::texreplace::complete(&config.paths.textures_dump(), &out))
}

#[tauri::command]
pub fn textures_open_folder(id: String) -> Result<(), String> {
    let (config, _) = crate::load().map_err(|e| format!("{e:#}"))?;
    let dir: PathBuf = if id.is_empty() {
        config.paths.textures_dump()
    } else {
        config.paths.textures_packs().join(&id)
    };
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("explorer");
        c.arg(&dir);
        c
    };
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = std::process::Command::new("open");
        c.arg(&dir);
        c
    };
    #[cfg(target_os = "linux")]
    let mut cmd = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(&dir);
        c
    };
    cmd.spawn()
        .map_err(|e| format!("open {}: {e}", dir.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const MANIFEST: &str = "\
name = \"Xbox Buttons\"
author = \"s2x3\"
version = \"1.0.0\"
kind = \"own-art\"
";

    fn entry_with(id: &str, err: Option<&str>, count: usize) -> s2x3_core::textures::PackEntry {
        let manifest = s2x3_core::textures::read_manifest;
        let _ = manifest;
        s2x3_core::textures::PackEntry {
            id: id.to_string(),
            pack: None,
            error: err.map(|e| e.to_string()),
            texture_count: count,
        }
    }

    #[test]
    fn dto_carries_state_and_errors() {
        let e = entry_with("xbox-buttons", None, 2);
        let d = dto_from(&e, Some(true), 0);
        assert_eq!(d.id, "xbox-buttons");
        assert!(d.enabled);
        assert_eq!(d.texture_count, 2);
        assert_eq!(d.name, "xbox-buttons", "fallback label without a pack");

        let e = entry_with("broken", Some("kaputt"), 0);
        let d = dto_from(&e, None, 1);
        assert_eq!(d.error.as_deref(), Some("kaputt"));
        assert!(!d.enabled);
        assert_eq!(d.conflicts, 1);
    }

    #[test]
    fn xbox_pack_manifest_is_written_once() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("xbox-buttons");
        fs::create_dir_all(&dir).unwrap();

        write_manifest_if_missing(&dir);
        let m = s2x3_core::textures::read_manifest(&dir).unwrap();
        assert_eq!(m.name, "Xbox Buttons");
        assert_eq!(m.kind, s2x3_core::textures::PackKind::Upscale);

        fs::write(
            &dir.join(s2x3_core::textures::MANIFEST),
            "name = \"x\"\nauthor = \"a\"\nversion = \"1\"\nkind = \"upscale\"\n",
        )
        .unwrap();
        write_manifest_if_missing(&dir);
        let m = s2x3_core::textures::read_manifest(&dir).unwrap();
        assert_eq!(m.name, "x", "an existing manifest is not overwritten");
    }
}

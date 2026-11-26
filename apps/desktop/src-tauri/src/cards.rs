//! Save profiles: named memory card images the launcher manages in
//! `saves/cards/`. The active one is what the game plays on; sharing packs a
//! card into a small `.ps2.gz` file (a 8 MB card compresses to ~44 KB) with
//! its SHA-256 fingerprint for comparison after transfer.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine as _;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::Serialize;
use sha2::{Digest, Sha256};
use s2x3_core::card::card_contents;

use crate::{load, Config};

/// Raw size of the standard 8 MB card image (528-byte pages).
const BLANK_CARD_BYTES: usize = 528 * 0x4000;

#[derive(Serialize, Debug)]
pub struct CardSaveDto {
    name: String,
    files: u32,
    bytes: u64,
}

#[derive(Serialize, Debug)]
pub struct CardProfile {
    name: String,
    path: String,
    /// File size in bytes; 0 when the card does not exist yet (blank until
    /// the game first writes to it).
    size: u64,
    /// Last write time as unix seconds; 0 when the file is missing.
    modified: i64,
    active: bool,
    exists: bool,
    /// Saves on the card (game save folders on the root).
    saves: Vec<CardSaveDto>,
    /// Total bytes of save data on the card.
    data_bytes: u64,
}

fn cards_dir(config: &Config) -> PathBuf {
    config.paths.saves.join("cards")
}

fn sanitize_name(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || " _-()".contains(c) {
                c
            } else {
                ' '
            }
        })
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        "save".to_string()
    } else {
        cleaned.to_string()
    }
}

fn profile_from(path: &Path, active: &Path) -> CardProfile {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("save")
        .to_string();
    let meta = std::fs::metadata(path).ok();
    let (size, modified, exists) = match &meta {
        Some(m) => (
            m.len(),
            m.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
            true,
        ),
        None => (0, 0, false),
    };
    let mut saves = Vec::new();
    let mut data_bytes = 0u64;
    if exists && size > 0 {
        if let Ok(contents) = card_contents(path) {
            data_bytes = contents.data_bytes;
            for e in contents.entries.iter().filter(|e| e.is_dir) {
                saves.push(CardSaveDto {
                    name: e.name.clone(),
                    files: e.length,
                    bytes: e.bytes,
                });
            }
        }
    }
    CardProfile {
        name,
        path: path.display().to_string(),
        size,
        modified,
        active: path == active,
        exists,
        saves,
        data_bytes,
    }
}

fn list_profiles(config: &Config) -> Vec<CardProfile> {
    let active = config.paths.active_memcard();
    let mut out = Vec::new();
    let dir = cards_dir(config);
    if let Ok(rd) = std::fs::read_dir(&dir) {
        let mut paths: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("ps2")))
            .collect();
        paths.sort();
        for p in paths {
            out.push(profile_from(&p, &active));
        }
    }
    // The classic card next to the saves is a profile too, unless the
    // active card already points at it (then it is listed above).
    let default = config.paths.saves.join("ssx3-mc0.ps2");
    let default_active = active == default;
    if !out.iter().any(|p| Path::new(&p.path) == default) && default.exists() {
        let mut p = profile_from(&default, &active);
        p.active = default_active;
        out.push(p);
    }
    if !out.iter().any(|p| p.active) {
        // The active card always shows up, even without a file yet.
        out.push(CardProfile {
            name: active
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("save")
                .to_string(),
            path: active.display().to_string(),
            size: 0,
            modified: 0,
            active: true,
            exists: false,
            saves: vec![],
            data_bytes: 0,
        });
    }
    out
}

fn load_config() -> Result<Config, String> {
    load().map(|(c, _)| c).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub fn save_profiles() -> Result<Vec<CardProfile>, String> {
    let config = load_config()?;
    Ok(list_profiles(&config))
}

fn card_path(config: &Config, name: &str) -> Result<PathBuf, String> {
    Ok(cards_dir(config).join(format!("{}.ps2", sanitize_name(name))))
}

/// Resolve a profile name to its real file: first the cards folder, then the
/// classic card next to the saves, then the active memcard path.
fn resolve_card(config: &Config, name: &str) -> Result<PathBuf, String> {
    let in_cards = card_path(config, name)?;
    if in_cards.exists() {
        return Ok(in_cards);
    }
    let classic = config.paths.saves.join(format!("{}.ps2", sanitize_name(name)));
    if classic.exists() {
        return Ok(classic);
    }
    let active = config.paths.active_memcard();
    if active.exists() && active.file_stem().and_then(|s| s.to_str()) == Some(sanitize_name(name).as_str()) {
        return Ok(active);
    }
    Err(format!("card not found: {name}"))
}

#[tauri::command]
pub fn card_new(name: String) -> Result<Vec<CardProfile>, String> {
    let config = load_config()?;
    let path = card_path(&config, &name)?;
    if path.exists() {
        return Err(format!("profile exists: {}", path.display()));
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    std::fs::write(&path, vec![0xFFu8; BLANK_CARD_BYTES])
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(list_profiles(&config))
}

#[tauri::command]
pub fn card_duplicate(name: String) -> Result<Vec<CardProfile>, String> {
    let config = load_config()?;
    let src = resolve_card(&config, &name)?;
    let dst = card_path(&config, &format!("{name} (Kopie)"))?;
    if dst.exists() {
        return Err(format!("profile exists: {}", dst.display()));
    }
    if let Some(dir) = dst.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    std::fs::copy(&src, &dst).map_err(|e| format!("copy: {e}"))?;
    Ok(list_profiles(&config))
}

#[tauri::command]
pub fn card_rename(name: String, new_name: String) -> Result<Vec<CardProfile>, String> {
    let mut config = load_config()?;
    let src = resolve_card(&config, &name)?;
    let dst = card_path(&config, &new_name)?;
    if dst.exists() {
        return Err(format!("profile exists: {}", dst.display()));
    }
    if let Some(dir) = dst.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    std::fs::rename(&src, &dst).map_err(|e| format!("rename: {e}"))?;
    // The classic card is implicitly active while memcard is unset: keep it
    // the active one after the rename.
    let is_default_active = config.paths.memcard.as_os_str().is_empty()
        && src == config.paths.saves.join("ssx3-mc0.ps2");
    if config.paths.memcard == src || is_default_active {
        config.paths.memcard = dst.clone();
        config.save().map_err(|e| format!("{e:#}"))?;
    }
    Ok(list_profiles(&config))
}

#[tauri::command]
pub fn card_delete(name: String) -> Result<Vec<CardProfile>, String> {
    let mut config = load_config()?;
    let path = resolve_card(&config, &name)?;
    std::fs::remove_file(&path).map_err(|e| format!("delete: {e}"))?;
    // Deleting the active card falls back to the classic default.
    if config.paths.memcard == path {
        config.paths.memcard = PathBuf::new();
        config.save().map_err(|e| format!("{e:#}"))?;
    }
    Ok(list_profiles(&config))
}

#[tauri::command]
pub fn card_activate(name: String) -> Result<Vec<CardProfile>, String> {
    let mut config = load_config()?;
    let path = resolve_card(&config, &name)?;
    config.paths.memcard = path;
    config.save().map_err(|e| format!("{e:#}"))?;
    Ok(list_profiles(&config))
}

#[tauri::command]
pub fn card_activate_default() -> Result<Vec<CardProfile>, String> {
    let mut config = load_config()?;
    config.paths.memcard = PathBuf::new();
    config.save().map_err(|e| format!("{e:#}"))?;
    Ok(list_profiles(&config))
}

fn gzip(data: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut enc = GzEncoder::new(Vec::new(), Compression::best());
    std::io::Write::write_all(&mut enc, data)?;
    enc.finish().map_err(Into::into)
}

fn gunzip(data: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut out = Vec::new();
    GzDecoder::new(data).read_to_end(&mut out)?;
    Ok(out)
}

fn sha8(data: &[u8]) -> String {
    Sha256::digest(data)[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[derive(Serialize)]
pub struct CardShare {
    file: String,
    /// SHA-256 of the compressed payload (16 hex chars) to compare after a
    /// transfer.
    hash: String,
}

/// Export a card as a small `.ps2.gz` the user can send over a messenger.
#[tauri::command]
pub async fn card_export(
    name: String,
    dest: Option<String>,
) -> Result<CardShare, String> {
    let path = resolve_card(&load_config()?, &name)?;
    let bytes =
        std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let packed = gzip(&bytes).map_err(|e| format!("{e:#}"))?;
    let hash = sha8(&packed);
    let target = match dest {
        Some(p) => PathBuf::from(p),
        None => rfd::AsyncFileDialog::new()
            .set_file_name(format!("{}.ps2.gz", sanitize_name(&name)))
            .save_file()
            .await
            .ok_or_else(|| "cancelled".to_string())?
            .path()
            .to_path_buf(),
    };
    std::fs::write(&target, &packed)
        .map_err(|e| format!("write {}: {e}", target.display()))?;
    Ok(CardShare {
        file: target.display().to_string(),
        hash,
    })
}

/// Import a shared `.ps2.gz` (or a raw `.ps2` card image) as a new profile.
#[tauri::command]
pub async fn card_import_file(
    src: Option<String>,
    name: Option<String>,
) -> Result<Vec<CardProfile>, String> {
    let path = match src {
        Some(p) => PathBuf::from(p),
        None => rfd::AsyncFileDialog::new()
            .add_filter("Save file", &["gz", "ps2"])
            .pick_file()
            .await
            .ok_or_else(|| "cancelled".to_string())?
            .path()
            .to_path_buf(),
    };
    let packed =
        std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let data = if packed.starts_with(b"\x1f\x8b") {
        gunzip(&packed).map_err(|e| format!("{e:#}"))?
    } else {
        packed
    };
    if !data.starts_with(b"Sony PS2 Memory Card Format ") {
        return Err("not a memory card image (missing superblock)".into());
    }
    let config = load_config()?;
    let suggested = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("imported")
        .trim_end_matches(".ps2")
        .to_string();
    let target_name = sanitize_name(
        name.as_deref()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or(&suggested),
    );
    let target = card_path(&config, &target_name)?;
    if target.exists() {
        return Err(format!(
            "profile exists: {} — rename it first or pick another file",
            target.display()
        ));
    }
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    std::fs::write(&target, &data).map_err(|e| format!("write {}: {e}", target.display()))?;
    Ok(list_profiles(&config))
}

/// The SHA-256 fingerprint (16 hex chars) of a card image, shown after an
/// import so it can be compared with the sender's.
#[tauri::command]
pub fn card_hash(name: String) -> Result<String, String> {
    let path = resolve_card(&load_config()?, &name)?;
    let bytes = std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    Ok(sha8(&bytes))
}

/// The whole card as a copyable payload string (no URL, no server):
/// `n=<name>&h=<sha8>&d=<gzip+base64>` — a 8 MB card packs into ~58 KB.
#[tauri::command]
pub fn card_hash_copy(name: String) -> Result<String, String> {
    let path = resolve_card(&load_config()?, &name)?;
    let bytes = std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let packed = gzip(&bytes).map_err(|e| format!("{e:#}"))?;
    Ok(format!(
        "n={}&h={}&d={}",
        sanitize_name(&name).replace(' ', "%20"),
        sha8(&packed),
        B64.encode(&packed)
    ))
}

/// Import a card from a pasted payload string (`…&d=…`), a full link with a
/// fragment, or a file path to a `.ps2.gz`/`.ps2`.
#[tauri::command]
pub fn card_import(payload: String, name: Option<String>) -> Result<Vec<CardProfile>, String> {
    let (data, suggested) = decode_payload(&payload)?;
    let config = load_config()?;
    let target_name = sanitize_name(
        name.as_deref()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or(&suggested),
    );
    let path = card_path(&config, &target_name)?;
    if path.exists() {
        return Err(format!("profile exists: {} — pick another name", path.display()));
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    std::fs::write(&path, &data).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(list_profiles(&config))
}

fn decode_payload(payload: &str) -> Result<(Vec<u8>, String), String> {
    let payload = payload.trim();
    // A link may carry the payload in its fragment; strip everything before it.
    let body = payload.split('#').nth(1).unwrap_or(payload);
    if body.contains("d=") {
        let mut data: Option<Vec<u8>> = None;
        let mut suggested = "imported".to_string();
        for part in body.split('&') {
            let Some((k, v)) = part.split_once('=') else {
                continue;
            };
            match k {
                "d" => {
                    let packed = B64
                        .decode(v.trim())
                        .map_err(|e| format!("payload is not decodable: {e}"))?;
                    data = Some(gunzip(&packed).map_err(|e| format!("{e:#}"))?);
                }
                "n" => suggested = percent_decode(v),
                _ => {}
            }
        }
        let Some(data) = data else {
            return Err("payload has no save data (missing d=)".into());
        };
        if !data.starts_with(b"Sony PS2 Memory Card Format ") {
            return Err("payload is not a memory card image".into());
        }
        return Ok((data, suggested));
    }
    // Otherwise: a file path to a .ps2.gz or raw .ps2.
    let path = PathBuf::from(body);
    let packed = std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let bytes = if packed.starts_with(b"\x1f\x8b") {
        gunzip(&packed).map_err(|e| format!("{e:#}"))?
    } else {
        packed
    };
    if !bytes.starts_with(b"Sony PS2 Memory Card Format ") {
        return Err("not a memory card image (missing superblock)".into());
    }
    let suggested = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("imported")
        .trim_end_matches(".ps2")
        .to_string();
    Ok((bytes, suggested))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() + 1 {
            if let Ok(v) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i + 1..(i + 3).min(bytes.len())]).unwrap_or(""),
                16,
            ) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("s2x3-cards-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("saves")).unwrap();
        let cfg = dir.join("config.toml");
        std::fs::write(
            &cfg,
            format!(
                "[engine]\nid = \"ps2-rust\"\nroot = \"\"\n\n[paths]\niso = \"\"\nsaves = '{}'\n",
                dir.join("saves").display().to_string().replace('\\', "/")
            ),
        )
        .unwrap();
        std::env::set_var("S2X3_CONFIG", &cfg);
        dir
    }

    #[test]
    fn new_export_import_roundtrip() {
        setup();
        std::fs::create_dir_all(cards_dir(&load_config().unwrap())).unwrap();
        let profiles = card_new("test run".into()).unwrap();
        let p = profiles.iter().find(|p| p.name == "test run").unwrap();
        assert!(p.exists && p.size as usize == BLANK_CARD_BYTES, "{p:?}");
        assert!(p.saves.is_empty(), "blank card has no saves");

        // put a formatted card in place and export it
        let path = card_path(&load_config().unwrap(), "test run").unwrap();
        let seeded = {
            let mut v = vec![0u8; BLANK_CARD_BYTES];
            v[..31].copy_from_slice(b"Sony PS2 Memory Card Format 1.2");
            v
        };
        std::fs::write(&path, &seeded).unwrap();
        let dest = std::env::temp_dir().join(format!("s2x3-export-{}.ps2.gz", std::process::id()));
        let share = tauri::async_runtime::block_on(card_export(
            "test run".into(),
            Some(dest.display().to_string()),
        ))
        .unwrap();
        assert!(Path::new(&share.file).exists());
        assert_eq!(share.hash.len(), 16);

        // importing the exported file under another name yields identical bytes
        let imported = tauri::async_runtime::block_on(card_import_file(
            Some(share.file.clone()),
            Some("imported".into()),
        ))
        .unwrap();
        let dst = card_path(&load_config().unwrap(), "imported").unwrap();
        assert!(imported.iter().any(|p| p.name == "imported"));
        assert_eq!(std::fs::read(&dst).unwrap(), seeded);

        // corrupt payload is rejected
        let bad = dest.with_file_name("bad.ps2.gz");
        std::fs::write(&bad, b"not gzip").unwrap();
        assert!(tauri::async_runtime::block_on(card_import_file(
            Some(bad.display().to_string()),
            Some("bad".into())
        ))
        .is_err());

        // the copyable hash payload round-trips too
        let payload = card_hash_copy("test run".into()).unwrap();
        assert!(payload.starts_with("n=test%20run&h="));
        card_import(payload, Some("hashcopy".into())).unwrap();
        let via_hash = card_path(&load_config().unwrap(), "hashcopy").unwrap();
        assert_eq!(std::fs::read(&via_hash).unwrap(), seeded);

        // rename keeps activation, delete falls back to default
        card_activate("test run".into()).unwrap();
        let p2 = card_rename("test run".into(), "renamed".into()).unwrap();
        assert!(p2.iter().any(|p| p.name == "renamed" && p.active));
        let cfg = load_config().unwrap();
        assert_eq!(cfg.paths.memcard, card_path(&cfg, "renamed").unwrap());
        card_delete("renamed".into()).unwrap();
        assert_eq!(load_config().unwrap().paths.memcard, PathBuf::new());
        let _ = std::fs::remove_dir_all(
            std::env::temp_dir().join(format!("s2x3-cards-test-{}", std::process::id())),
        );
        let _ = std::fs::remove_file(&dest);
        let _ = std::fs::remove_file(&bad);
    }
}

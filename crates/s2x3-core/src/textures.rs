//! Texture packs: manifest parsing and listing.
//!
//! Spec: docs/superpowers/specs/2026-11-26-texture-packs-design.md — a pack
//! is a folder with `manifest.toml` and `textures/<hash>.png` files. Hash
//! names are 16 lowercase hex digits (the engine's `pack::name` output).

use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// The manifest file name inside a pack folder.
pub const MANIFEST: &str = "manifest.toml";

/// What a pack's textures are. `OwnArt` packs are distributable; `Upscale`
/// packs are derived from the user's own disc and never leave the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackKind {
    OwnArt,
    Upscale,
}

impl fmt::Display for PackKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackKind::OwnArt => f.write_str("own-art"),
            PackKind::Upscale => f.write_str("upscale"),
        }
    }
}

/// `manifest.toml` of a pack.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PackManifest {
    pub name: String,
    pub author: String,
    pub version: String,
    pub kind: PackKind,
    /// Engine commit the pack was built against (staleness check).
    #[serde(default)]
    pub engine_commit: Option<String>,
    /// Hash (16 hex digits) → human label of what the texture shows.
    #[serde(default)]
    pub targets: BTreeMap<String, String>,
}

/// A parsed pack: an id (folder name), its folder and manifest.
#[derive(Debug, Clone)]
pub struct Pack {
    pub id: String,
    pub dir: PathBuf,
    pub manifest: PackManifest,
}

/// One folder in the packs directory, valid or not.
#[derive(Debug, Clone)]
pub struct PackEntry {
    pub id: String,
    pub pack: Option<Pack>,
    /// Why the folder is not a usable pack (or has suspicious contents), if
    /// it is not.
    pub error: Option<String>,
    /// Number of `textures/*.png` files in the folder.
    pub texture_count: usize,
}

/// Read and validate `manifest.toml` inside `dir`.
pub fn read_manifest(dir: &Path) -> Result<PackManifest> {
    let path = dir.join(MANIFEST);
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

/// True when `stem` looks like an engine texture name: 16 hex digits.
fn valid_hash(stem: &str) -> bool {
    stem.len() == 16 && stem.chars().all(|c| c.is_ascii_hexdigit())
}

fn png_count(textures_dir: &Path) -> usize {
    std::fs::read_dir(textures_dir)
        .map(|rd| {
            rd.flatten()
                .filter(|f| {
                    f.path()
                        .extension()
                        .is_some_and(|x| x.eq_ignore_ascii_case("png"))
                })
                .count()
        })
        .unwrap_or(0)
}

/// Stems of the pack's `textures/*.png` that are not valid engine hashes.
fn bad_hash_names(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir.join("textures"))
        .map(|rd| {
            rd.flatten()
                .filter_map(|f| {
                    let p = f.path();
                    let stem = p.file_stem()?.to_str()?.to_string();
                    (!valid_hash(&stem)).then_some(stem)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// List the packs in `packs_dir`, sorted by id. Folders that are not usable
/// packs come back with `error` set and `pack: None` — they are shown, never
/// hidden, and never modified by listing.
pub fn list(packs_dir: &Path) -> Vec<PackEntry> {
    let mut out = Vec::new();
    let Ok(read) = std::fs::read_dir(packs_dir) else {
        return out;
    };
    for e in read.flatten() {
        let dir = e.path();
        if !dir.is_dir() {
            continue;
        }
        let id = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        let texture_count = png_count(&dir.join("textures"));
        match read_manifest(&dir) {
            Ok(manifest) => {
                let bad = bad_hash_names(&dir);
                let error = if bad.is_empty() {
                    None
                } else {
                    Some(format!(
                        "texture names are not engine hashes: {}",
                        bad.join(", ")
                    ))
                };
                out.push(PackEntry {
                    pack: Some(Pack {
                        id: id.clone(),
                        dir,
                        manifest,
                    }),
                    id,
                    error,
                    texture_count,
                });
            }
            Err(e) => out.push(PackEntry {
                id,
                pack: None,
                error: Some(format!("{e:#}")),
                texture_count,
            }),
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const VALID: &str = "\
name = \"Xbox Buttons\"
author = \"s2x3\"
version = \"1.0.0\"
kind = \"own-art\"

[targets]
\"f897162c0f661c8b\" = \"Steuerkreuz-Atlas\"
";

    fn pack(dir: &Path, id: &str, manifest: &str, textures: &[&str]) -> PathBuf {
        let p = dir.join(id);
        fs::create_dir_all(p.join("textures")).unwrap();
        fs::write(p.join(MANIFEST), manifest).unwrap();
        for t in textures {
            fs::write(p.join("textures").join(t), b"png").unwrap();
        }
        p
    }

    #[test]
    fn parses_a_valid_manifest() {
        let tmp = TempDir::new().unwrap();
        let dir = pack(tmp.path(), "xbox-buttons", VALID, &[]);
        let m = read_manifest(&dir).unwrap();
        assert_eq!(m.name, "Xbox Buttons");
        assert_eq!(m.author, "s2x3");
        assert_eq!(m.version, "1.0.0");
        assert_eq!(m.kind, PackKind::OwnArt);
        assert_eq!(m.engine_commit, None);
        assert_eq!(m.targets.len(), 1);
    }

    #[test]
    fn missing_field_is_an_error() {
        let tmp = TempDir::new().unwrap();
        let no_name = "author = \"a\"\nversion = \"1\"\nkind = \"upscale\"\n";
        let dir = pack(tmp.path(), "broken", no_name, &[]);
        let err = format!("{:#}", read_manifest(&dir).unwrap_err());
        assert!(err.contains("missing field `name`"), "got: {err}");
        assert!(err.contains("manifest.toml"), "got: {err}");
    }

    #[test]
    fn unknown_kind_is_an_error() {
        let tmp = TempDir::new().unwrap();
        let bad = "name = \"n\"\nauthor = \"a\"\nversion = \"1\"\nkind = \"wallpaper\"\n";
        let dir = pack(tmp.path(), "weird", bad, &[]);
        let err = format!("{:#}", read_manifest(&dir).unwrap_err());
        assert!(err.contains("wallpaper"), "got: {err}");
    }

    #[test]
    fn list_sorts_and_flags_broken_and_bad_hashes() {
        let tmp = TempDir::new().unwrap();
        let packs = tmp.path();
        pack(packs, "b-valid", VALID, &["f897162c0f661c8b.png"]);
        pack(
            packs,
            "a-badname",
            VALID,
            &["notahash.png", "0123456789abcdef.png"],
        );
        fs::create_dir_all(packs.join("c-empty")).unwrap();

        let entries = list(packs);
        let ids: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["a-badname", "b-valid", "c-empty"]);

        let bad = &entries[0];
        assert!(bad.pack.is_some());
        assert!(
            bad.error.as_deref().unwrap_or("").contains("notahash"),
            "got: {:?}",
            bad.error
        );
        assert_eq!(bad.texture_count, 2);

        assert!(entries[1].pack.is_some());
        assert!(entries[1].error.is_none());
        assert_eq!(entries[1].texture_count, 1);

        assert!(entries[2].pack.is_none());
        assert!(entries[2].error.is_some());
        assert_eq!(entries[2].texture_count, 0);
    }

    #[test]
    fn list_is_empty_without_the_folder() {
        let tmp = TempDir::new().unwrap();
        assert!(list(&tmp.path().join("missing")).is_empty());
    }
}

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

/// Turn a folder name into a pack id: lowercase, only `[a-z0-9._-]`.
fn sanitize_id(raw: &str) -> String {
    let mut id: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    while id.starts_with('.') {
        id.remove(0);
    }
    id
}

/// Unpack the zip in `src` (or copy the folder `src`) into `packs_dir`.
/// Returns the new pack's id. With `overwrite = false` an existing pack id
/// is an error; the existing folder is never touched by a failed import.
pub fn import(src: &Path, packs_dir: &Path, overwrite: bool) -> Result<String> {
    std::fs::create_dir_all(packs_dir)
        .with_context(|| format!("create {}", packs_dir.display()))?;
    let staging = packs_dir.join(format!(".import-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).with_context(|| format!("create {}", staging.display()))?;
    let result = extract_into(src, &staging).and_then(|id| {
        read_manifest(&staging)
            .with_context(|| format!("no usable {MANIFEST} in the imported pack"))?;
        install(&staging, packs_dir, &id, overwrite)?;
        Ok(id)
    });
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    result
}

/// Move the staged pack into place: existing folder aside, staging in, old
/// folder gone (rolled back on error).
fn install(staging: &Path, packs_dir: &Path, id: &str, overwrite: bool) -> Result<()> {
    let target = packs_dir.join(id);
    if target.exists() {
        if !overwrite {
            anyhow::bail!("pack `{id}` exists already; overwrite is off");
        }
        let old = packs_dir.join(format!(".old-{id}"));
        let _ = std::fs::remove_dir_all(&old);
        std::fs::rename(&target, &old)
            .with_context(|| format!("move aside {}", target.display()))?;
        match std::fs::rename(&staging, &target) {
            Ok(()) => {
                let _ = std::fs::remove_dir_all(&old);
                Ok(())
            }
            Err(e) => {
                let _ = std::fs::rename(&old, &target);
                Err(anyhow::Error::new(e).context(format!("swap in {}", target.display())))
            }
        }
    } else {
        std::fs::rename(&staging, &target).with_context(|| format!("install {}", target.display()))
    }
}

/// Extract a zip file or copy a folder into `staging`; returns the pack id.
fn extract_into(src: &Path, staging: &Path) -> Result<String> {
    if src.is_dir() {
        let raw = src
            .file_name()
            .and_then(|n| n.to_str())
            .context("import path has no file name")?;
        let id = sanitize_id(raw);
        copy_tree(src, staging).with_context(|| format!("copy {}", src.display()))?;
        return Ok(id);
    }
    let file = std::fs::File::open(src).with_context(|| format!("read {}", src.display()))?;
    let mut archive =
        zip::ZipArchive::new(file).with_context(|| format!("open zip {}", src.display()))?;
    if archive.len() == 0 {
        anyhow::bail!("zip {} is empty", src.display());
    }
    // One root folder holding the pack; its name is the id.
    let mut root: Option<String> = None;
    let mut total: u64 = 0;
    for i in 0..archive.len() {
        let mut f = archive
            .by_index(i)
            .with_context(|| format!("zip entry {i} of {}", src.display()))?;
        let Some(rel) = f.enclosed_name().map(|p| p.to_path_buf()) else {
            let name = f
                .name()
                .map(|n| n.to_string())
                .unwrap_or_else(|_| format!("entry {i}"));
            anyhow::bail!("zip entry {name} escapes the pack folder");
        };
        let top = rel
            .components()
            .next()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .unwrap_or_default();
        match &root {
            None => root = Some(top.clone()),
            Some(r) if *r != top => anyhow::bail!(
                "zip {} holds more than one root folder (`{r}` vs `{top}`)",
                src.display()
            ),
            _ => {}
        }
        if top.is_empty() || rel.as_os_str().is_empty() {
            anyhow::bail!("zip {} has entries outside a pack folder", src.display());
        }
        // Staging receives the pack contents directly; the root folder only
        // names the pack.
        let rel_noroot: PathBuf = rel.components().skip(1).collect();
        if f.is_dir() {
            std::fs::create_dir_all(staging.join(&rel_noroot))
                .with_context(|| format!("create {}", rel.display()))?;
            continue;
        }
        total += f.size();
        if total > 512 * 1024 * 1024 {
            anyhow::bail!("zip {} is larger than the 512 MB pack limit", src.display());
        }
        let dst = staging.join(&rel_noroot);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        let mut out =
            std::fs::File::create(&dst).with_context(|| format!("create {}", dst.display()))?;
        std::io::copy(&mut f, &mut out).with_context(|| format!("extract {}", dst.display()))?;
    }
    let root = root.unwrap_or_default();
    let id = sanitize_id(&root);
    if id.is_empty() {
        anyhow::bail!("zip {} has no usable pack folder name", src.display());
    }
    Ok(id)
}

/// Copy a folder tree recursively (files and empty folders).
fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst).with_context(|| format!("create {}", dst.display()))?;
    for e in std::fs::read_dir(src).with_context(|| format!("read {}", src.display()))? {
        let e = e?;
        let from = e.path();
        let to = dst.join(e.file_name());
        if from.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).with_context(|| format!("copy {}", from.display()))?;
        }
    }
    Ok(())
}

/// Delete the pack `id` from `packs_dir`.
pub fn remove(packs_dir: &Path, id: &str) -> Result<()> {
    if sanitize_id(id) != id || id.is_empty() {
        anyhow::bail!("`{id}` is not a pack name");
    }
    let target = packs_dir.join(id);
    if !target.is_dir() {
        anyhow::bail!("pack `{id}` is not installed in {}", packs_dir.display());
    }
    std::fs::remove_dir_all(&target).with_context(|| format!("remove {}", target.display()))
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

    // --- import / remove -------------------------------------------------

    use std::io::Cursor;
    use std::io::Write as _;

    /// Build a zip file in memory from (name, content) entries.
    fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = Cursor::new(Vec::new());
        let mut z = zip::ZipWriter::new(&mut w);
        for (name, data) in entries {
            z.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(data).unwrap();
        }
        z.finish().unwrap();
        w.into_inner()
    }

    fn valid_zip() -> Vec<u8> {
        zip_bytes(&[
            ("xbox-buttons/manifest.toml", VALID.as_bytes()),
            (
                "xbox-buttons/textures/f897162c0f661c8b.png",
                b"png".as_slice(),
            ),
        ])
    }

    #[test]
    fn import_unzips_a_valid_pack() {
        let tmp = TempDir::new().unwrap();
        let packs = tmp.path().join("packs");
        let src = tmp.path().join("pack.zip");
        fs::write(&src, valid_zip()).unwrap();

        let id = import(&src, &packs, false).unwrap();
        assert_eq!(id, "xbox-buttons");
        let entries = list(&packs);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "xbox-buttons");
        assert!(entries[0].pack.is_some());
        assert_eq!(entries[0].texture_count, 1);
    }

    #[test]
    fn import_rejects_zip_slip() {
        let tmp = TempDir::new().unwrap();
        let packs = tmp.path().join("packs");
        let src = tmp.path().join("evil.zip");
        fs::write(
            &src,
            zip_bytes(&[
                ("p/manifest.toml", VALID.as_bytes()),
                ("p/../evil.txt", b"x".as_slice()),
            ]),
        )
        .unwrap();

        let err = format!("{:#}", import(&src, &packs, false).unwrap_err());
        assert!(err.contains("evil.txt"), "got: {err}");
        assert!(!tmp.path().join("evil.txt").exists());
        assert!(!packs.join("p").exists());
    }

    #[test]
    fn import_rejects_missing_manifest() {
        let tmp = TempDir::new().unwrap();
        let packs = tmp.path().join("packs");
        let src = tmp.path().join("nomani.zip");
        fs::write(
            &src,
            zip_bytes(&[("p/textures/f897162c0f661c8b.png", b"png".as_slice())]),
        )
        .unwrap();

        let err = format!("{:#}", import(&src, &packs, false).unwrap_err());
        assert!(err.contains("manifest.toml"), "got: {err}");
    }

    #[test]
    fn import_over_existing_id_needs_overwrite() {
        let tmp = TempDir::new().unwrap();
        let packs = tmp.path().join("packs");
        let src = tmp.path().join("pack.zip");
        fs::write(&src, valid_zip()).unwrap();
        import(&src, &packs, false).unwrap();
        fs::write(
            packs
                .join("xbox-buttons")
                .join("textures")
                .join("0123456789abcdef.png"),
            b"old",
        )
        .unwrap();

        let err = format!("{:#}", import(&src, &packs, false).unwrap_err());
        assert!(err.contains("xbox-buttons"), "got: {err}");
        // the old folder is untouched by the failed import
        assert!(packs
            .join("xbox-buttons")
            .join("textures")
            .join("0123456789abcdef.png")
            .is_file());

        import(&src, &packs, true).unwrap();
        assert!(!packs
            .join("xbox-buttons")
            .join("textures")
            .join("0123456789abcdef.png")
            .is_file());
    }

    #[test]
    fn import_accepts_uppercase_names_and_imports_folders() {
        let tmp = TempDir::new().unwrap();
        let packs = tmp.path().join("packs");
        let src_dir = tmp.path().join("My Pack");
        fs::create_dir_all(src_dir.join("textures")).unwrap();
        fs::write(src_dir.join(MANIFEST), VALID).unwrap();
        fs::write(
            src_dir.join("textures").join("F897162C0F661C8B.PNG"),
            b"png",
        )
        .unwrap();

        let id = import(&src_dir, &packs, false).unwrap();
        assert_eq!(id, "my-pack");
        let entries = list(&packs);
        assert_eq!(entries[0].id, "my-pack");
        assert_eq!(entries[0].texture_count, 1);
        assert!(entries[0].error.is_none());
    }

    #[test]
    fn remove_deletes_only_the_named_pack() {
        let tmp = TempDir::new().unwrap();
        let packs = tmp.path().join("packs");
        fs::create_dir_all(packs.join("a").join("textures")).unwrap();
        fs::create_dir_all(packs.join("b")).unwrap();

        remove(&packs, "a").unwrap();
        assert!(!packs.join("a").exists());
        assert!(packs.join("b").exists());

        let err = format!("{:#}", remove(&packs, "gone").unwrap_err());
        assert!(err.contains("gone"), "got: {err}");
        // traversal is rejected, not followed
        let err = format!("{:#}", remove(&packs, "../b").unwrap_err());
        assert!(err.contains("b") || err.contains("name"), "got: {err}");
        assert!(packs.join("b").exists());
    }
}

//! Texture packs: manifest parsing and listing.
//!
//! Spec: docs/superpowers/specs/2026-11-26-texture-packs-design.md â€” a pack
//! is a folder with `manifest.toml` and `textures/<hash>.png` files. Hash
//! names are 16 lowercase hex digits (the engine's `pack::name` output).

use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fmt;
use std::io::Read;
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
    /// Hash (16 hex digits) â†’ human label of what the texture shows.
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
/// packs come back with `error` set and `pack: None` â€” they are shown, never
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

/// The engine binary, same resolution as the adapters: a built binary wins
/// over the dist fallback.
pub fn engine_binary(config: &crate::Config) -> PathBuf {
    let exe = format!("ssx3{}", std::env::consts::EXE_SUFFIX);
    let built = config.engine.root.join("target/game").join(&exe);
    if built.exists() {
        return built;
    }
    config.engine.root.join("dist").join(exe)
}

/// Run the offline extraction (`ssx3 dump-textures`) against the configured
/// ISO into `paths.textures/dump`. Needs an engine with `dump-textures`
/// support; the error names the binary when it is missing.
pub fn run_extract(config: &crate::Config) -> Result<String> {
    let bin = engine_binary(config);
    if !bin.exists() {
        anyhow::bail!(
            "engine binary missing: {} — build it first; the offline \
             extraction needs an engine with `dump-textures` support",
            bin.display()
        );
    }
    let out = config.paths.textures_dump();
    std::fs::create_dir_all(&out).with_context(|| format!("create {}", out.display()))?;
    let res = std::process::Command::new(&bin)
        .arg("dump-textures")
        .arg("--iso")
        .arg(&config.paths.iso)
        .arg("--out")
        .arg(&out)
        .output()
        .with_context(|| format!("run {} dump-textures", bin.display()))?;
    if !res.status.success() {
        let tail = String::from_utf8_lossy(&res.stderr);
        let mut tail = tail
            .lines()
            .rev()
            .take(3)
            .collect::<Vec<_>>()
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        tail.reverse();
        anyhow::bail!(
            "dump-textures failed: {}",
            if tail.is_empty() {
                format!("exit code {:?}", res.status.code())
            } else {
                tail.join(" | ")
            }
        );
    }
    let summary = String::from_utf8_lossy(&res.stdout);
    Ok(summary
        .lines()
        .last()
        .unwrap_or("extraction finished")
        .to_string())
}

/// Texture hashes that more than one *enabled* pack claims — read-only (the
/// merge itself is what resolves them; this is for the UI's warning).
pub fn conflicts(
    packs_dir: &Path,
    enabled: &BTreeMap<String, bool>,
    priority: &[String],
) -> Vec<String> {
    let _ = priority;
    let mut claim: BTreeMap<String, usize> = BTreeMap::new();
    for e in list(packs_dir) {
        if !enabled.get(&e.id).copied().unwrap_or(false) {
            continue;
        }
        let tex_dir = packs_dir.join(&e.id).join("textures");
        if let Ok(rd) = std::fs::read_dir(tex_dir) {
            for f in rd.flatten() {
                let p = f.path();
                if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("png")) {
                    if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                        *claim.entry(stem.to_ascii_lowercase()).or_default() += 1;
                    }
                }
            }
        }
    }
    claim
        .into_iter()
        .filter(|(_, n)| *n > 1)
        .map(|(h, _)| h)
        .collect()
}

/// The python launcher for the current platform (checked, not spawned).
fn python() -> &'static str {
    if cfg!(windows) {
        "python"
    } else {
        "python3"
    }
}

/// Run the ESRGAN upscale over the extracted dump into the `hd-upscale`
/// pack folder, then manifest it as a pack. Needs Python and a
/// Vulkan-capable GPU; errors name what is missing.
pub fn run_upscale(config: &crate::Config) -> Result<PathBuf> {
    let tool = config.engine.root.join("tools").join("upscale-textures.py");
    if !tool.is_file() {
        anyhow::bail!(
            "upscale-textures tool missing in the engine checkout: {} — \
             update the engine",
            tool.display()
        );
    }
    let pack = config.paths.textures_packs().join("hd-upscale");
    std::fs::create_dir_all(&pack).with_context(|| format!("create {}", pack.display()))?;
    let status = std::process::Command::new(python())
        .arg(&tool)
        .arg("--in")
        .arg(config.paths.textures_dump())
        .arg("--out")
        .arg(pack.join("textures"))
        .arg("--esrgan")
        .arg(config.paths.textures.join(".tools"))
        .status();
    match status {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            anyhow::bail!("python not found — install Python 3 (the engine setup needs it too)")
        }
        Err(e) => return Err(anyhow::Error::new(e).context("start python")),
        Ok(s) if !s.success() => {
            anyhow::bail!("upscale-textures failed (exit code {:?}) — its log lines above say what went wrong", s.code());
        }
        Ok(_) => {}
    }
    std::fs::write(
        pack.join(MANIFEST),
        format!(
            "name = \"HD Upscale\"\nauthor = \"s2x3\"\nversion = \"1.0.0\"\nkind = \"upscale\"\n"
        ),
    )
    .with_context(|| format!("write {}", pack.join(MANIFEST).display()))?;
    Ok(pack)
}

/// A stable marker for the configured ISO: its size and sha1, streamed.
/// Used to decide whether the offline extraction has to run again.
pub fn iso_marker(config: &crate::Config) -> Result<String> {
    use sha1::{Digest, Sha1};
    let path = &config.paths.iso;
    let mut f = std::fs::File::open(path).with_context(|| format!("read {}", path.display()))?;
    let mut hasher = Sha1::new();
    let mut buf = [0u8; 1024 * 1024];
    let mut len: u64 = 0;
    loop {
        let n = f
            .read(&mut buf)
            .with_context(|| format!("read {}", path.display()))?;
        if n == 0 {
            break;
        }
        len += n as u64;
        hasher.update(&buf[..n]);
    }
    Ok(format!(
        "{}-{}",
        len,
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    ))
}

/// Outcome of a merge: how many replacement files are active, and which
/// texture hashes more than one enabled pack claims (first in priority
/// wins; the conflict is surfaced, never hidden).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeReport {
    pub files: u32,
    pub conflicts: Vec<String>,
}

/// Rebuild the `active` merge folder from the enabled packs: copied files of
/// every enabled pack, in `priority` order (earlier wins a hash), remaining
/// ids after that in sort order. The folder is rebuilt from scratch each
/// time, so a stale or corrupted `active` heals on the next merge.
pub fn merge(
    packs_dir: &Path,
    active_dir: &Path,
    enabled: &BTreeMap<String, bool>,
    priority: &[String],
) -> Result<MergeReport> {
    // Enabled, installed packs in application order.
    let entries = list(packs_dir);
    let mut order: Vec<&PackEntry> = Vec::new();
    for id in priority {
        if enabled.get(id).copied().unwrap_or(false) {
            if let Some(e) = entries.iter().filter(|e| e.id == *id).next() {
                order.push(e);
            }
        }
    }
    for e in &entries {
        if enabled.get(&e.id).copied().unwrap_or(false) && !priority.iter().any(|p| p == &e.id) {
            order.push(e);
        }
    }

    let staging = active_dir.with_file_name(format!(
        "{}.tmp",
        active_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("active")
    ));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).with_context(|| format!("create {}", staging.display()))?;
    let mut seen: Vec<String> = Vec::new();
    let mut conflicts: Vec<String> = Vec::new();
    let mut files: u32 = 0;
    for entry in &order {
        let pack = match &entry.pack {
            Some(p) => p,
            None => continue, // broken folder: listed in the UI, skipped here
        };
        let tex_dir = pack.dir.join("textures");
        let Ok(rd) = std::fs::read_dir(&tex_dir) else {
            continue;
        };
        for f in rd.flatten() {
            let path = f.path();
            if !path
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("png"))
            {
                continue;
            }
            let stem = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s.to_ascii_lowercase(),
                None => continue,
            };
            if seen.iter().any(|h| h == &stem) {
                conflicts.push(stem.clone());
                continue;
            }
            seen.push(stem.clone());
            let dst = staging.join(format!("{stem}.png"));
            std::fs::copy(&path, &dst)
                .with_context(|| format!("copy {} into the active set", path.display()))?;
            files += 1;
        }
    }
    conflicts.sort();
    conflicts.dedup();

    // Atomic swap: old folder aside, staging in, old gone; roll back on error.
    if active_dir.exists() {
        let old = active_dir.with_file_name(format!(
            "{}.old",
            active_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("active")
        ));
        let _ = std::fs::remove_dir_all(&old);
        std::fs::rename(active_dir, &old)
            .with_context(|| format!("move aside {}", active_dir.display()))?;
        match std::fs::rename(&staging, active_dir) {
            Ok(()) => {
                let _ = std::fs::remove_dir_all(&old);
            }
            Err(e) => {
                let _ = std::fs::rename(&old, active_dir);
                let _ = std::fs::remove_dir_all(&staging);
                return Err(
                    anyhow::Error::new(e).context(format!("swap in {}", active_dir.display()))
                );
            }
        }
    } else {
        std::fs::rename(&staging, active_dir)
            .with_context(|| format!("activate {}", active_dir.display()))?;
    }
    Ok(MergeReport { files, conflicts })
}

/// Value for `SSX3_TEX_REPLACE`: the active merge folder when it holds at
/// least one replacement, else `"0"` (explicitly off â€” the engine's default
/// `textures/replace` folder must never apply behind the launcher's back).
pub fn launch_replace_env(config: &crate::Config) -> String {
    let active = config.paths.textures_active();
    let any = png_count(&active) > 0;
    if any {
        active.display().to_string()
    } else {
        "0".to_string()
    }
}

const LEGACY_MANIFEST: &str = "\
name = \"Xbox prompts (imported)\"
author = \"s2x3\"
version = \"1.0.0\"
kind = \"upscale\"
";

/// One-time migration from the pre-pack mechanism: `ui.buttons = "xbox"`
/// becomes the `xbox-buttons` pack toggle, and a filled legacy
/// `paths.texreplace` folder is imported as the `texreplace-import` pack
/// (derived content: local-only, kind `upscale`). Returns `Ok(true)` when
/// the config changed; idempotent â€” a second run changes nothing.
pub fn migrate_legacy(config: &mut crate::Config) -> Result<bool> {
    let mut changed = false;
    if config.ui.buttons == "xbox" && config.textures.enabled.get("xbox-buttons") != Some(&true) {
        config.textures.enabled.insert("xbox-buttons".into(), true);
        changed = true;
    }
    let src = config.paths.texreplace.clone();
    let has_pngs = png_count(&src) > 0;
    let dst = config.paths.textures_packs().join("texreplace-import");
    if has_pngs && !dst.exists() {
        std::fs::create_dir_all(dst.join("textures"))
            .with_context(|| format!("create {}", dst.display()))?;
        for e in std::fs::read_dir(&src).with_context(|| format!("read {}", src.display()))? {
            let e = e.with_context(|| format!("read {}", src.display()))?;
            let p = e.path();
            if !p.extension().is_some_and(|x| x.eq_ignore_ascii_case("png")) {
                continue;
            }
            std::fs::copy(&p, dst.join("textures").join(e.file_name()))
                .with_context(|| format!("copy {}", p.display()))?;
        }
        std::fs::write(dst.join(MANIFEST), LEGACY_MANIFEST)
            .with_context(|| format!("write {}", dst.join(MANIFEST).display()))?;
        config
            .textures
            .enabled
            .insert("texreplace-import".into(), true);
        changed = true;
    }
    Ok(changed)
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

    // --- merge -----------------------------------------------------------

    const HASH1: &str = "f897162c0f661c8b";
    const HASH2: &str = "75df99fccd7d9d0b";

    fn enabled(ids: &[&str]) -> BTreeMap<String, bool> {
        ids.iter().map(|id| (id.to_string(), true)).collect()
    }

    #[test]
    fn merge_picks_priority_order_and_reports_conflicts() {
        let tmp = TempDir::new().unwrap();
        let packs = tmp.path().join("packs");
        pack(packs.as_path(), "a", VALID, &[&format!("{HASH1}.png")]);
        pack(
            packs.as_path(),
            "b",
            VALID,
            &[&format!("{HASH1}.png"), &format!("{HASH2}.png")],
        );
        // distinct content so the winner is identifiable
        fs::write(
            packs
                .join("a")
                .join("textures")
                .join(format!("{HASH1}.png")),
            b"png-1",
        )
        .unwrap();

        let active = tmp.path().join("active");
        let report = merge(
            &packs,
            &active,
            &enabled(&["b", "a"]),
            &["a".into(), "b".into()],
        )
        .unwrap();

        let win1 = fs::read(active.join(format!("{HASH1}.png"))).unwrap();
        assert_eq!(win1, b"png-1".to_vec(), "priority `a` must win HASH1");
        assert!(active.join(format!("{HASH2}.png")).is_file());
        assert_eq!(report.files, 2);
        assert_eq!(report.conflicts, vec![HASH1.to_string()]);
    }

    #[test]
    fn merge_skips_disabled_and_rebuilds_clean() {
        let tmp = TempDir::new().unwrap();
        let packs = tmp.path().join("packs");
        pack(packs.as_path(), "a", VALID, &[&format!("{HASH1}.png")]);
        pack(packs.as_path(), "b", VALID, &[&format!("{HASH2}.png")]);
        let active = tmp.path().join("active");
        fs::create_dir_all(&active).unwrap();
        fs::write(active.join("stale.png"), b"stale").unwrap();

        let mut on = enabled(&["a", "b"]);
        on.insert("b".into(), false);
        merge(&packs, &active, &on, &[]).unwrap();

        assert!(active.join(format!("{HASH1}.png")).is_file());
        assert!(!active.join(format!("{HASH2}.png")).is_file());
        assert!(!active.join("stale.png").is_file(), "rebuild starts clean");
    }

    #[test]
    fn merge_ignores_missing_enabled_packs() {
        let tmp = TempDir::new().unwrap();
        let packs = tmp.path().join("packs");
        pack(packs.as_path(), "a", VALID, &[&format!("{HASH1}.png")]);
        let active = tmp.path().join("active");

        let mut on = enabled(&["a", "ghost"]);
        on.insert("ghost".into(), true);
        let report = merge(&packs, &active, &on, &["ghost".into(), "a".into()]).unwrap();
        assert_eq!(report.files, 1);
        assert!(active.join(format!("{HASH1}.png")).is_file());
    }

    // --- launch env / legacy migration -----------------------------------

    use crate::Config;

    #[test]
    fn launch_env_is_zero_without_active_packs() {
        let tmp = TempDir::new().unwrap();
        let mut cfg = Config::default();
        cfg.paths.textures = tmp.path().join("textures");
        assert_eq!(launch_replace_env(&cfg), "0");
    }

    #[test]
    fn launch_env_names_active_when_packs_are_merged() {
        let tmp = TempDir::new().unwrap();
        let mut cfg = Config::default();
        cfg.paths.textures = tmp.path().join("textures");
        std::fs::create_dir_all(cfg.paths.textures_active()).unwrap();
        std::fs::write(
            cfg.paths.textures_active().join("f897162c0f661c8b.png"),
            b"png",
        )
        .unwrap();
        // a non-png file does not count as an active pack
        std::fs::write(cfg.paths.textures_active().join("readme.txt"), b"n").unwrap();
        let env = launch_replace_env(&cfg);
        assert!(env.ends_with("active"), "got: {env}");
        assert_ne!(env, "0");
    }

    #[test]
    fn migration_is_idempotent_and_imports_texreplace() {
        let tmp = TempDir::new().unwrap();
        let mut cfg = Config::default();
        cfg.paths.textures = tmp.path().join("textures");
        cfg.paths.texreplace = tmp.path().join("texreplace");
        std::fs::create_dir_all(&cfg.paths.texreplace).unwrap();
        std::fs::write(cfg.paths.texreplace.join("f897162c0f661c8b.png"), b"png").unwrap();
        cfg.ui.buttons = "xbox".into();

        assert!(migrate_legacy(&mut cfg).unwrap());
        assert_eq!(cfg.textures.enabled.get("xbox-buttons"), Some(&true));
        // the legacy folder stays in place (it still works until the next
        // config save swaps the mechanism) and a copy became a pack
        assert!(cfg.paths.texreplace.join("f897162c0f661c8b.png").is_file());
        let entries = list(&cfg.paths.textures_packs());
        assert!(
            entries
                .iter()
                .any(|e| e.id == "texreplace-import" && e.pack.is_some()),
            "got: {entries:?}"
        );
        assert_eq!(cfg.textures.enabled.get("texreplace-import"), Some(&true));

        assert!(!migrate_legacy(&mut cfg).unwrap());
    }

    #[test]
    fn conflicts_reports_shared_hashes_of_enabled_packs() {
        let tmp = TempDir::new().unwrap();
        let packs = tmp.path().join("packs");
        pack(
            packs.as_path(),
            "a",
            VALID,
            &[&format!("{HASH1}.png"), &format!("{HASH2}.png")],
        );
        pack(packs.as_path(), "b", VALID, &[&format!("{HASH1}.png")]);

        let mut on = enabled(&["a", "b"]);
        assert_eq!(conflicts(&packs, &on, &[]), vec![HASH1.to_string()]);
        on.insert("b".into(), false);
        assert!(conflicts(&packs, &on, &[]).is_empty());
    }

    // --- engine tooling ---------------------------------------------------

    #[test]
    fn engine_binary_prefers_the_built_over_the_dist_one() {
        let tmp = TempDir::new().unwrap();
        let mut cfg = Config::default();
        cfg.engine.root = tmp.path().to_path_buf();
        let exe = format!("ssx3{}", std::env::consts::EXE_SUFFIX);
        std::fs::create_dir_all(tmp.path().join("dist")).unwrap();
        std::fs::write(tmp.path().join("dist").join(&exe), b"x").unwrap();
        assert_eq!(engine_binary(&cfg), tmp.path().join("dist").join(&exe));
        std::fs::create_dir_all(tmp.path().join("target/game")).unwrap();
        std::fs::write(tmp.path().join("target/game").join(&exe), b"x").unwrap();
        assert_eq!(
            engine_binary(&cfg),
            tmp.path().join("target/game").join(&exe)
        );
    }

    #[test]
    fn extract_fails_readable_without_an_engine_binary() {
        let tmp = TempDir::new().unwrap();
        let mut cfg = Config::default();
        cfg.engine.root = tmp.path().join("no-engine");
        cfg.paths.iso = tmp.path().join("iso");
        cfg.paths.textures = tmp.path().join("textures");
        let err = format!("{:#}", run_extract(&cfg).unwrap_err());
        assert!(err.contains("dump-textures"), "got: {err}");
        assert!(err.contains("engine binary missing"), "got: {err}");
    }

    #[test]
    fn upscale_fails_readable_without_the_tool() {
        let tmp = TempDir::new().unwrap();
        let mut cfg = Config::default();
        cfg.engine.root = tmp.path().to_path_buf();
        cfg.paths.textures = tmp.path().join("textures");
        let err = format!("{:#}", run_upscale(&cfg).unwrap_err());
        assert!(err.contains("upscale-textures"), "got: {err}");
    }
}

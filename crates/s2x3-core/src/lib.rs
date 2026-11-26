//! S2X3 core: models and the engine adapter contract.
//!
//! The launcher never links an engine. It writes settings, checks paths and
//! starts a child process. Everything engine specific lives behind
//! [`EngineAdapter`], so the Xbox recomp (or a web build) can be added later
//! without touching the UI.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Child;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub mod card;
pub mod textures;

pub const CONFIG_ENV: &str = "S2X3_CONFIG";

/// What an engine can do. The UI shows fields and buttons from this, so a
/// new engine does not need UI code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    pub platforms: Vec<String>,
    pub needs_disc: bool,
    pub plugins: bool,
    pub netplay: bool,
    pub splitscreen: bool,
    pub max_tick_hz: u32,
    pub notes: Vec<String>,
}

impl Default for Capabilities {
    fn default() -> Self {
        Self {
            platforms: vec![],
            needs_disc: true,
            plugins: false,
            netplay: false,
            splitscreen: false,
            max_tick_hz: 60,
            notes: vec![],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Bool,
    Integer { min: i64, max: i64 },
    Choice(&'static [&'static str]),
    Text,
}

#[derive(Debug, Clone)]
pub struct SettingsField {
    pub key: &'static str,
    pub label: &'static str,
    pub help: &'static str,
    pub kind: FieldKind,
    pub default: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Ready(String),
    Missing(String),
}

impl Status {
    pub fn is_ready(&self) -> bool {
        matches!(self, Status::Ready(_))
    }
}

#[derive(Debug, Clone)]
pub struct Detection {
    pub engine: Status,
    pub disc: Status,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct LaunchConfig {
    /// Settings for this run only, on top of the saved file.
    pub overrides: BTreeMap<String, String>,
    /// Raw environment for this run (advanced/diagnostic keys that are not
    /// part of the launcher's schema). Not validated.
    pub extra_env: BTreeMap<String, String>,
    pub extra_args: Vec<String>,
    /// Capture the engine's stdout/stderr here instead of inheriting them.
    pub log: Option<std::path::PathBuf>,
}

/// One installed engine. Engines are addressed by id (`ps2-rust`,
/// `xbox-recomp`, ...).
pub trait EngineAdapter {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> Capabilities;
    fn detect(&self) -> Detection;
    fn settings_schema(&self) -> &'static [SettingsField];

    /// Settings as the engine sees them: file first, then the launcher's
    /// overrides.
    fn read_settings(&self) -> Result<BTreeMap<String, String>>;
    fn write_settings(&self, values: &BTreeMap<String, String>) -> Result<()>;

    /// Start the engine. The caller owns the child (stdio, wait, logs).
    fn launch(&self, config: &LaunchConfig) -> Result<Child>;

    /// Where the engine keeps its saves and whether they exist.
    fn saves_info(&self) -> Result<SavesInfo> {
        anyhow::bail!("this engine has no save file")
    }

    /// Copy the engine's save file to `dest` (a file or an existing folder).
    fn export_saves(&self, _dest: &Path) -> Result<PathBuf> {
        anyhow::bail!("this engine cannot export saves")
    }

    /// Replace the engine's save file with `src`, after backing up the old one.
    fn import_saves(&self, _src: &Path) -> Result<PathBuf> {
        anyhow::bail!("this engine cannot import saves")
    }

    /// Build/install the engine from its sources (optional). Returns a child
    /// whose output the caller may capture.
    fn install(&self) -> Result<Child> {
        anyhow::bail!("this engine has no installer")
    }
}

/// On-disk launcher config: where the engine, the disc and the saves are.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub engine: EngineRef,
    #[serde(default)]
    pub paths: Paths,
    /// Launcher-side defaults, written into the engine's settings file.
    #[serde(default)]
    pub settings: BTreeMap<String, String>,
    /// Launcher behaviour that is not an engine setting.
    #[serde(default)]
    pub ui: Ui,
    /// Texture pack state: which packs are on, their order, and whether the
    /// offline extraction has run for the current ISO.
    #[serde(default)]
    pub textures: TexturesSettings,
}

/// Texture pack state kept in the config (spec: textures design).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TexturesSettings {
    /// Pack id → enabled. Absent = not installed.
    #[serde(default)]
    pub enabled: BTreeMap<String, bool>,
    /// Pack ids first-in-list wins a shared texture hash.
    #[serde(default)]
    pub priority: Vec<String>,
    /// Hash of the ISO the offline extraction ran against ("" = never).
    #[serde(default)]
    pub iso_marker: String,
    /// The legacy texreplace migration ran once; deletions of the imported
    /// pack stay deleted.
    #[serde(default)]
    pub legacy_migrated: bool,
    /// The xbox-buttons pack was generated once from the dump; deleting the
    /// pack is not undone by the next launch (re-enabled by re-picking Xbox
    /// prompts, which resets this).
    #[serde(default)]
    pub generated_xbox: bool,
}

/// Launcher UI preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ui {
    /// Which button prompts to show and inject: "xbox" or "ps2".
    #[serde(default = "default_buttons")]
    pub buttons: String,
}

fn default_buttons() -> String {
    "xbox".into()
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            buttons: default_buttons(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineRef {
    pub id: String,
    pub root: PathBuf,
}

impl Default for EngineRef {
    fn default() -> Self {
        Self {
            id: "ps2-rust".into(),
            root: PathBuf::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Paths {
    pub iso: PathBuf,
    pub saves: PathBuf,
    /// Texture replacement folder for the Xbox button prompts.
    #[serde(default)]
    pub texreplace: PathBuf,
    /// The active memory card image (save profile). Empty means the classic
    /// `saves/ssx3-mc0.ps2`. [carried uncommitted from the parallel agent's
    /// in-flight save-manager work; base commit e943a94 needs it]
    #[serde(default)]
    pub memcard: PathBuf,
    /// Root of the launcher-managed texture pack store: `dump/` (extracted
    /// game textures), `packs/<id>/` (installed packs), `active/` (merge
    /// folder handed to the engine at launch).
    #[serde(default)]
    pub textures: PathBuf,
}

/// `$S2X3_CONFIG`, else `%APPDATA%/s2x3/config.toml` on Windows, else
/// `$XDG_CONFIG_HOME/s2x3/config.toml`, else `~/.config/s2x3/config.toml`.
pub fn config_path() -> PathBuf {
    if let Some(p) = std::env::var_os(CONFIG_ENV) {
        return PathBuf::from(p);
    }
    #[cfg(target_os = "windows")]
    if let Some(dir) = std::env::var_os("APPDATA") {
        return PathBuf::from(dir).join("s2x3").join("config.toml");
    }
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(dir).join("s2x3").join("config.toml");
    }
    let home = std::env::var_os("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join(".config")
        .join("s2x3")
        .join("config.toml")
}

/// Configuration path used before the project was renamed. Read once so an
/// existing installation keeps its engine, disc and save paths.
fn legacy_config_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    if let Some(dir) = std::env::var_os("APPDATA") {
        return PathBuf::from(dir).join("slope3").join("config.toml");
    }
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(dir).join("slope3").join("config.toml");
    }
    let home = std::env::var_os("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join(".config")
        .join("slope3")
        .join("config.toml")
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path();
        if !path.exists() && std::env::var_os(CONFIG_ENV).is_none() {
            let old = legacy_config_path();
            if old.exists() {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::copy(&old, &path);
            }
        }
        Self::load_from(&path)
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&config_path())
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        let text = toml::to_string_pretty(self)?;
        std::fs::write(path, text).with_context(|| format!("write {}", path.display()))
    }

    /// Fill in what the user did not set: saves next to the config, engine
    /// root from the repository location.
    pub fn resolved(&self) -> Self {
        let mut c = self.clone();
        if c.paths.saves.as_os_str().is_empty() {
            let base = config_path()
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            c.paths.saves = base.join("saves");
        }
        if c.paths.texreplace.as_os_str().is_empty() {
            let base = c
                .paths
                .saves
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            c.paths.texreplace = base.join("texreplace");
        }
        if c.paths.textures.as_os_str().is_empty() {
            let base = c
                .paths
                .saves
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            c.paths.textures = base.join("textures");
        }
        c
    }
}

impl Paths {
    /// The memory card the game plays on; `memcard` when set, else the
    /// classic `ssx3-mc0.ps2` next to the saves.
    pub fn active_memcard(&self) -> PathBuf {
        if self.memcard.as_os_str().is_empty() {
            self.saves.join("ssx3-mc0.ps2")
        } else {
            self.memcard.clone()
        }
    }
    /// Extracted game textures (offline dump or `SSX3_TEX_DUMP` output).
    pub fn textures_dump(&self) -> PathBuf {
        self.textures.join("dump")
    }
    /// Installed texture packs, one folder per pack.
    pub fn textures_packs(&self) -> PathBuf {
        self.textures.join("packs")
    }
    /// Merge folder of the enabled packs; handed to the engine as
    /// `SSX3_TEX_REPLACE`.
    pub fn textures_active(&self) -> PathBuf {
        self.textures.join("active")
    }
}

/// A save file the launcher can back up or restore.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavesInfo {
    pub path: PathBuf,
    pub exists: bool,
    pub size: u64,
}

/// Human readable "1.4 GB" for the UI/log.
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024.0 && u + 1 < UNITS.len() {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_fills_texture_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg_path = tmp.path().join("config.toml");
        std::env::set_var(CONFIG_ENV, &cfg_path);
        let base = tmp.path();

        let c = Config::default().resolved();

        assert_eq!(c.paths.textures, base.join("textures"));
        assert_eq!(c.paths.textures_dump(), base.join("textures").join("dump"));
        assert_eq!(
            c.paths.textures_packs(),
            base.join("textures").join("packs")
        );
        assert_eq!(
            c.paths.textures_active(),
            base.join("textures").join("active")
        );
    }
}

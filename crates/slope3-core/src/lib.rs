//! Slope3 core: models and the engine adapter contract.
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

pub const CONFIG_ENV: &str = "SLOPE3_CONFIG";

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
}

/// `$SLOPE3_CONFIG`, else `$XDG_CONFIG_HOME/slope3/config.toml`, else
/// `~/.config/slope3/config.toml`.
pub fn config_path() -> PathBuf {
    if let Some(p) = std::env::var_os(CONFIG_ENV) {
        return PathBuf::from(p);
    }
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(dir).join("slope3").join("config.toml");
    }
    let home = std::env::var_os("HOME").unwrap_or_default();
    PathBuf::from(home).join(".config").join("slope3").join("config.toml")
}

impl Config {
    pub fn load() -> Result<Self> {
        Self::load_from(&config_path())
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read {}", path.display()))?;
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
        c
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

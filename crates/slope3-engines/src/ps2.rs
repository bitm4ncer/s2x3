//! The PlayStation 2 / Rust engine (`ssx3-decomp`).
//!
//! It is a normal desktop program: settings live in
//! `saves/ssx3-settings.cfg`, the disc image is passed as `SSX3_ISO`, the
//! memory card as `SSX3_MEMCARD`. The launcher writes those and starts the
//! binary; nothing is linked.

use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use anyhow::{Context, Result};
use slope3_core::{
    human_size, Capabilities, Detection, EngineAdapter, LaunchConfig, SettingsField, Status,
    FieldKind,
};

pub const ID: &str = "ps2-rust";

static SCHEMA: &[SettingsField] = &[
    SettingsField {
        key: "SSX3_TICK_HZ",
        label: "Updates per second",
        help: "Game updates per second. 60 is the original; 120/144/240 give smoother motion on fast displays.",
        kind: FieldKind::Integer { min: 60, max: 240 },
        default: "60",
    },
    SettingsField {
        key: "SSX3_RENDER_SCALE",
        label: "Internal resolution",
        help: "N times the game's resolution (1-8). The window size stays independent.",
        kind: FieldKind::Integer { min: 1, max: 8 },
        default: "1",
    },
    SettingsField {
        key: "SSX3_ASPECT",
        label: "Aspect",
        help: "Fixed aspect instead of following the window. Empty: follow the window.",
        kind: FieldKind::Choice(&["", "4:3", "16:9", "stretch"]),
        default: "",
    },
    SettingsField {
        key: "SSX3_RENDER_SIZE",
        label: "Render size",
        help: "Fixed output size, e.g. 2560x1080. Empty: follow the window.",
        kind: FieldKind::Text,
        default: "",
    },
    SettingsField {
        key: "SSX3_FOV",
        label: "Field of view",
        help: "Relative to the game's own FOV (1.0 = original, 1.2 = 20% wider).",
        kind: FieldKind::Text,
        default: "1.0",
    },
    SettingsField {
        key: "SSX3_ANISO",
        label: "Anisotropic filtering",
        help: "1 (off) to 16, for textures with mipmaps.",
        kind: FieldKind::Integer { min: 1, max: 16 },
        default: "16",
    },
    SettingsField {
        key: "SSX3_SKIP_MOVIES",
        label: "Skip intro movies",
        help: "Skip the EA, THX and intro movies at boot.",
        kind: FieldKind::Bool,
        default: "0",
    },
    SettingsField {
        key: "SSX3_REAL_CLOCK",
        label: "Use the host clock",
        help: "Real time of day and save timestamps instead of the fixed clock.",
        kind: FieldKind::Bool,
        default: "0",
    },
    SettingsField {
        key: "SSX3_AA",
        label: "Anti-aliasing",
        help: "Modern renderer anti-aliasing (off by default).",
        kind: FieldKind::Bool,
        default: "0",
    },
    SettingsField {
        key: "SSX3_AO",
        label: "Ambient occlusion",
        help: "Modern renderer ambient occlusion (off by default).",
        kind: FieldKind::Bool,
        default: "0",
    },
    SettingsField {
        key: "SSX3_BLOOM",
        label: "Bloom",
        help: "Modern renderer bloom (off by default).",
        kind: FieldKind::Bool,
        default: "0",
    },
    SettingsField {
        key: "SSX3_SHADOWS",
        label: "Real-time lighting",
        help: "Experimental: terrain lit by the renderer with sun shadows.",
        kind: FieldKind::Bool,
        default: "0",
    },
    SettingsField {
        key: "SSX3_PAD2",
        label: "Second controller",
        help: "Plug in pad 2 for Multi Play.",
        kind: FieldKind::Bool,
        default: "0",
    },
    SettingsField {
        key: "SSX3_FULLSCREEN",
        label: "Fullscreen",
        help: "Start borderless fullscreen (F11 toggles while playing).",
        kind: FieldKind::Bool,
        default: "0",
    },
];

fn known(key: &str) -> bool {
    SCHEMA.iter().any(|f| f.key == key)
}

pub struct Ps2Rust {
    config: slope3_core::Config,
}

impl Ps2Rust {
    pub fn new(config: slope3_core::Config) -> Self {
        Self { config }
    }

    pub fn binary(&self) -> PathBuf {
        let root = &self.config.engine.root;
        let built = root.join("target/game/ssx3");
        if built.exists() {
            return built;
        }
        root.join("dist/ssx3")
    }

    pub fn settings_path(&self) -> PathBuf {
        self.config.paths.saves.join("ssx3-settings.cfg")
    }

    pub fn memcard_path(&self) -> PathBuf {
        self.config.paths.saves.join("ssx3-mc0.ps2")
    }
}

impl EngineAdapter for Ps2Rust {
    fn id(&self) -> &'static str {
        ID
    }

    fn name(&self) -> &'static str {
        "SSX 3 (PlayStation 2, Rust port)"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            platforms: vec!["linux".into(), "windows".into(), "macos".into()],
            needs_disc: true,
            plugins: true,
            netplay: false,
            splitscreen: true,
            max_tick_hz: 240,
            notes: vec![
                "PS2 USA disc (SLUS_207.72)".into(),
                "Plugins can add courses from Tricky and On Tour".into(),
            ],
        }
    }

    fn detect(&self) -> Detection {
        let mut notes = Vec::new();

        let binary = self.binary();
        let engine = if binary.exists() {
            Status::Ready(format!("{}", binary.display()))
        } else {
            Status::Missing(format!(
                "{} (build it with `cargo build --profile game -p ssx3`)",
                binary.display()
            ))
        };

        let iso = &self.config.paths.iso;
        let disc = match std::fs::metadata(iso) {
            Ok(m) if m.len() > 1_000_000_000 => {
                let serial = iso_serial(iso).unwrap_or_else(|| "unknown serial".into());
                Status::Ready(format!("{serial}, {}", human_size(m.len())))
            }
            Ok(m) => Status::Missing(format!(
                "{} is only {} - not a disc image",
                iso.display(),
                human_size(m.len())
            )),
            Err(_) => Status::Missing(format!("{} not found", iso.display())),
        };
        if let Ok(m) = std::fs::metadata(iso) {
            if m.len() < 4_500_000_000 {
                notes.push("Looks like a PS2 single-layer disc (good).".into());
            }
        }
        notes.push(format!("Settings: {}", self.settings_path().display()));
        notes.push(format!("Memory card: {}", self.memcard_path().display()));

        Detection {
            engine,
            disc,
            notes,
        }
    }

    fn settings_schema(&self) -> &'static [SettingsField] {
        SCHEMA
    }

    fn read_settings(&self) -> Result<BTreeMap<String, String>> {
        let mut out = BTreeMap::new();
        let path = self.settings_path();
        if !path.exists() {
            return Ok(out);
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let k = k.trim();
                if known(k) {
                    out.insert(k.to_string(), v.trim().to_string());
                }
            }
        }
        Ok(out)
    }

    fn write_settings(&self, values: &BTreeMap<String, String>) -> Result<()> {
        let path = self.settings_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        let mut text = String::from(
            "# Written by Slope3. Lines are KEY=VALUE; environment variables win.\n",
        );
        for field in SCHEMA {
            if let Some(v) = values.get(field.key) {
                if !v.is_empty() {
                    text.push_str(&format!("{}={}\n", field.key, v));
                }
            }
        }
        std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))
    }

    fn install(&self) -> Result<Child> {
        let root = &self.config.engine.root;
        if !root.join("tools/setup.py").is_file() {
            anyhow::bail!(
                "{} is not an ssx3-decomp checkout (tools/setup.py missing)",
                root.display()
            );
        }
        std::fs::create_dir_all(&self.config.paths.saves)
            .with_context(|| format!("create {}", self.config.paths.saves.display()))?;
        let log_path = self.config.paths.saves.join("install.log");
        let out = std::fs::File::create(&log_path)
            .with_context(|| format!("create {}", log_path.display()))?;
        let err = out.try_clone().context("clone install log")?;
        let mut cmd = Command::new("bash");
        cmd.arg("-lc")
            .arg("python3 tools/setup.py && cargo build --profile game -p ssx3")
            .current_dir(root)
            .env("SSX3_ISO", &self.config.paths.iso)
            .stdout(out)
            .stderr(err);
        cmd.spawn()
            .with_context(|| format!("start the engine build in {}", root.display()))
    }

    fn launch(&self, config: &LaunchConfig) -> Result<Child> {
        let binary = self.binary();
        if !binary.exists() {
            anyhow::bail!(
                "engine binary missing: {} (build it first)",
                binary.display()
            );
        }
        std::fs::create_dir_all(&self.config.paths.saves)
            .with_context(|| format!("create {}", self.config.paths.saves.display()))?;
        self.write_settings(&self.config.settings)?;

        let mut cmd = Command::new(&binary);
        cmd.current_dir(&self.config.engine.root)
            .env("SSX3_ISO", &self.config.paths.iso)
            .env("SSX3_MEMCARD", self.memcard_path())
            .env("SSX3_SETTINGS", self.settings_path());

        for (k, v) in &self.config.settings {
            cmd.env(k, v);
        }
        for (k, v) in &config.overrides {
            cmd.env(k, v);
        }
        for (k, v) in &config.extra_env {
            cmd.env(k, v);
        }
        cmd.args(&config.extra_args);

        if let Some(path) = &config.log {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let out = std::fs::File::create(path)
                .with_context(|| format!("create log {}", path.display()))?;
            let err = out.try_clone().context("clone log handle")?;
            cmd.stdout(out).stderr(err);
        }

        cmd.spawn()
            .with_context(|| format!("start {}", binary.display()))
    }
}

/// Read the boot serial from the ISO's first sectors: `SYSTEM.CNF` is one of
/// the first files, so a small scan finds `BOOT2=cdrom0:\\SLUS_207.72;1`.
pub fn iso_serial(path: &Path) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut buf = vec![0u8; 8 * 1024 * 1024];
    f.seek(SeekFrom::Start(0)).ok()?;
    let mut filled = 0usize;
    while filled < buf.len() {
        match f.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(_) => break,
        }
    }
    let text = String::from_utf8_lossy(&buf[..filled]);
    let idx = text.find("BOOT2=cdrom0:")?;
    let rest = &text[idx + "BOOT2=cdrom0:".len()..];
    let rest = rest.trim_start_matches(['\\', '/']);
    let serial: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '.' || *c == '-')
        .collect();
    let serial = serial.trim_end_matches(";1").to_string();
    (!serial.is_empty()).then_some(serial)
}

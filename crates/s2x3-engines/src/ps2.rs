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
use s2x3_core::{
    human_size, Capabilities, Detection, EngineAdapter, FieldKind, LaunchConfig, SavesInfo,
    SettingsField, Status,
};

/// A PS2 memory card image is exactly 8 MiB of card data plus its header.
const CARD_BYTES: u64 = 8_650_752;

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
        kind: FieldKind::Choice(&["", "4:3", "16:9", "stretch", "cover"]),
        default: "",
    },
    SettingsField {
        key: "SSX3_HUD_EDGE",
        label: "HUD edge slide",
        help: "How far the race HUD's anchored elements slide toward the frame edges (0-160, HUD pixels; the game's own transform squeezes the HUD, so ~96 moves an element by ~11% of the window). 0: as authored.",
        kind: FieldKind::Integer { min: 0, max: 160 },
        default: "0",
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
        default: "1",
    },
    SettingsField {
        key: "SSX3_AA",
        label: "Anti-aliasing",
        help: "Modern renderer anti-aliasing (off by default).",
        kind: FieldKind::Bool,
        default: "1",
    },
    SettingsField {
        key: "SSX3_AO",
        label: "Ambient occlusion",
        help: "Modern renderer ambient occlusion (off by default).",
        kind: FieldKind::Bool,
        default: "1",
    },
    SettingsField {
        key: "SSX3_BLOOM",
        label: "Bloom",
        help: "Modern renderer bloom (off by default).",
        kind: FieldKind::Bool,
        default: "1",
    },
    SettingsField {
        key: "SSX3_SHADOWS",
        label: "Real-time lighting",
        help: "Experimental: terrain lit by the renderer with sun shadows.",
        kind: FieldKind::Bool,
        default: "1",
    },
    SettingsField {
        key: "SSX3_PAD2",
        label: "Second controller",
        help: "Plug in pad 2 for Multi Play.",
        kind: FieldKind::Bool,
        default: "1",
    },
    SettingsField {
        key: "SSX3_STICK_TRICKS",
        label: "Trick Spin Controls",
        help: "Cross Pad is the original scheme. Left Stick or Right Stick also presses the d-pad, so trick combos work without reaching for it.",
        kind: FieldKind::Choice(&["", "left", "right"]),
        default: "",
    },
    SettingsField {
        key: "SSX3_FULLSCREEN",
        label: "Fullscreen",
        help: "Start borderless fullscreen (F11 toggles while playing).",
        kind: FieldKind::Bool,
        default: "1",
    },
    SettingsField {
        key: "SSX3_RESOLUTION",
        label: "Render resolution",
        help: "Render at a fraction of the window, then scale up. Lower is faster.",
        kind: FieldKind::Choice(&["", "75", "50"]),
        default: "",
    },
    SettingsField {
        key: "SSX3_LOD_BIAS",
        label: "Texture sharpness",
        help: "Mip bias: soft, standard, sharp or sharper.",
        kind: FieldKind::Choice(&["0.5", "", "-0.5", "-1"]),
        default: "",
    },
    SettingsField {
        key: "SSX3_SNOW",
        label: "Snow sparkle",
        help: "The glitter on the snow surface.",
        kind: FieldKind::Bool,
        default: "1",
    },
    SettingsField {
        key: "SSX3_CATCHUP",
        label: "Keep speed when slow",
        help: "On: the game catches up after a slow frame instead of slowing down.",
        kind: FieldKind::Choice(&["", "0"]),
        default: "",
    },
    SettingsField {
        key: "SSX3_FAST_MATH",
        label: "Fast math",
        help: "Cheaper floating point in the recompiled game code.",
        kind: FieldKind::Bool,
        default: "1",
    },
    SettingsField {
        key: "SSX3_RENDER",
        label: "Renderer",
        help: "Native wgpu renderer or the emulated GS.",
        kind: FieldKind::Choice(&["", "emu"]),
        default: "",
    },
    SettingsField {
        key: "SSX3_SND",
        label: "Sound",
        help: "Native mixer or the emulated IOP.",
        kind: FieldKind::Choice(&["", "iop"]),
        default: "",
    },
    SettingsField {
        key: "SSX3_IOP_SIO2",
        label: "Pads and memory card",
        help: "Native servers or the recompiled IOP modules.",
        kind: FieldKind::Choice(&["", "1"]),
        default: "",
    },
    SettingsField {
        key: "SSX3_NATIVE_FILES",
        label: "Game files",
        help: "Read files from the disc image natively or through the emulated CD.",
        kind: FieldKind::Choice(&["", "0"]),
        default: "",
    },
];

fn known(key: &str) -> bool {
    SCHEMA.iter().any(|f| f.key == key)
}

pub struct Ps2Rust {
    config: s2x3_core::Config,
}

impl Ps2Rust {
    pub fn new(config: s2x3_core::Config) -> Self {
        Self { config }
    }

    pub fn binary(&self) -> PathBuf {
        let root = &self.config.engine.root;
        let exe = format!("ssx3{}", std::env::consts::EXE_SUFFIX);
        let built = root.join("target/game").join(&exe);
        if built.exists() {
            return built;
        }
        root.join("dist").join(exe)
    }

    pub fn settings_path(&self) -> PathBuf {
        self.config.paths.saves.join("ssx3-settings.cfg")
    }

    pub fn memcard_path(&self) -> PathBuf {
        self.config.paths.active_memcard()
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
        let text =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
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
        // Keep keys the launcher does not know (the engine's own settings
        // file can contain more than this schema): read, overlay, write.
        let mut all: BTreeMap<String, String> = BTreeMap::new();
        if let Ok(text) = std::fs::read_to_string(&path) {
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = line.split_once('=') {
                    all.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
        }
        for (k, v) in values {
            if v.is_empty() {
                all.remove(k);
            } else {
                all.insert(k.clone(), v.clone());
            }
        }
        let mut text =
            String::from("# Written by S2X3. Lines are KEY=VALUE; environment variables win.\n");
        for (k, v) in &all {
            text.push_str(&format!("{k}={v}\n"));
        }
        std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))
    }

    fn saves_info(&self) -> Result<SavesInfo> {
        let path = self.memcard_path();
        let meta = std::fs::metadata(&path).ok();
        Ok(SavesInfo {
            path,
            exists: meta.is_some(),
            size: meta.map(|m| m.len()).unwrap_or(0),
        })
    }

    fn export_saves(&self, dest: &Path) -> Result<PathBuf> {
        let src = self.memcard_path();
        if !src.is_file() {
            anyhow::bail!("no memory card yet: {}", src.display());
        }
        let target = if dest.is_dir() {
            dest.join(src.file_name().context("memory card has no file name")?)
        } else {
            dest.to_path_buf()
        };
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        std::fs::copy(&src, &target)
            .with_context(|| format!("copy {} to {}", src.display(), target.display()))?;
        Ok(target)
    }

    fn import_saves(&self, src: &Path) -> Result<PathBuf> {
        let meta = std::fs::metadata(src).with_context(|| format!("read {}", src.display()))?;
        if meta.len() != CARD_BYTES {
            anyhow::bail!(
                "{} is not a PS2 memory card: expected {} bytes, got {}",
                src.display(),
                CARD_BYTES,
                meta.len()
            );
        }
        let dst = self.memcard_path();
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        if dst.exists() {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let backup = dst.with_extension(format!("ps2.bak-{stamp}"));
            std::fs::copy(&dst, &backup)
                .with_context(|| format!("back up {} to {}", dst.display(), backup.display()))?;
        }
        std::fs::copy(src, &dst)
            .with_context(|| format!("copy {} to {}", src.display(), dst.display()))?;
        Ok(dst)
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
        #[cfg(target_os = "windows")]
        let script = "python tools/setup.py\nif ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }\ncargo build --profile game -p ssx3";
        #[cfg(not(target_os = "windows"))]
        let script = "python3 tools/setup.py && cargo build --profile game -p ssx3";

        #[cfg(target_os = "windows")]
        let mut cmd = Command::new("powershell");
        #[cfg(target_os = "windows")]
        cmd.args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ]);
        #[cfg(not(target_os = "windows"))]
        let mut cmd = Command::new("bash");
        #[cfg(not(target_os = "windows"))]
        cmd.args(["-lc", script]);

        cmd.current_dir(root)
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
        // Merge, never overwrite: the engine's own overlay (F10) writes this
        // file too, and a launch must not wipe what the player changed.
        let mut values = self.read_settings().unwrap_or_default();
        for (k, v) in &self.config.settings {
            values.insert(k.clone(), v.clone());
        }
        self.write_settings(&values)?;

        let mut cmd = Command::new(&binary);
        // The engine binary is a console program: hide the console window
        // that Windows would otherwise attach for a GUI parent.
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        cmd.current_dir(&self.config.engine.root)
            .env("SSX3_ISO", &self.config.paths.iso)
            .env("SSX3_MEMCARD", self.memcard_path())
            .env("SSX3_SETTINGS", self.settings_path());
        // Always explicit: the engine's default `textures/replace` folder
        // must never apply behind the launcher's back (toggle off = original
        // look).
        cmd.env(
            "SSX3_TEX_REPLACE",
            s2x3_core::textures::launch_replace_env(&self.config),
        );
        // The dump is cumulative: every session adds the textures it uploads
        // (the engine writes each name once), so packs grow to cover
        // everything the player has seen without extra work.
        cmd.env("SSX3_TEX_DUMP", self.config.paths.textures_dump());

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

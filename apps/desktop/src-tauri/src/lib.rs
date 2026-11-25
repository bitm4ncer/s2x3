use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use s2x3_core::{
    Capabilities, Config, EngineAdapter, FieldKind, LaunchConfig, Status,
};
use s2x3_engines::by_id;
use tauri::Manager;

#[derive(Serialize)]
struct FieldDto {
    key: String,
    label: String,
    help: String,
    kind: String,
    choices: Vec<String>,
    choice_labels: Vec<String>,
    min: Option<i64>,
    max: Option<i64>,
    default: String,
}

#[derive(Serialize)]
struct StatusDto {
    state: String,
    detail: String,
}

#[derive(Serialize)]
struct CapsDto {
    platforms: Vec<String>,
    needs_disc: bool,
    plugins: bool,
    netplay: bool,
    splitscreen: bool,
    max_tick_hz: u32,
    notes: Vec<String>,
}

#[derive(Serialize)]
struct StateDto {
    version: String,
    config_path: String,
    engine_id: String,
    engine_name: String,
    engine_root: String,
    iso: String,
    saves: String,
    binary: StatusDto,
    disc: StatusDto,
    notes: Vec<String>,
    capabilities: CapsDto,
    schema: Vec<FieldDto>,
    values: BTreeMap<String, String>,
    buttons: String,
    texreplace: String,
    texreplace_present: bool,
    log_path: String,
}

fn load() -> anyhow::Result<(Config, Box<dyn EngineAdapter>)> {
    let config = Config::load()?.resolved();
    let engine = by_id(&config)?;
    Ok((config, engine))
}

fn status(s: &Status) -> StatusDto {
    match s {
        Status::Ready(detail) => StatusDto {
            state: "ready".into(),
            detail: detail.clone(),
        },
        Status::Missing(detail) => StatusDto {
            state: "missing".into(),
            detail: detail.clone(),
        },
    }
}

fn caps(c: &Capabilities) -> CapsDto {
    CapsDto {
        platforms: c.platforms.clone(),
        needs_disc: c.needs_disc,
        plugins: c.plugins,
        netplay: c.netplay,
        splitscreen: c.splitscreen,
        max_tick_hz: c.max_tick_hz,
        notes: c.notes.clone(),
    }
}

fn fields(engine: &dyn EngineAdapter) -> Vec<FieldDto> {
    engine
        .settings_schema()
        .iter()
        .map(|f| {
            let (kind, choices, min, max) = match f.kind {
                FieldKind::Bool => ("bool".to_string(), vec![], None, None),
                FieldKind::Integer { min, max } => ("int".to_string(), vec![], Some(min), Some(max)),
                FieldKind::Choice(c) => (
                    "choice".to_string(),
                    c.iter().map(|s| s.to_string()).collect(),
                    None,
                    None,
                ),
                FieldKind::Text => ("text".to_string(), vec![], None, None),
            };
            let labels: Option<&[&str]> = match f.key {
                "SSX3_STICK_TRICKS" => Some(&["Cross Pad (default)", "Left Stick", "Right Stick"]),
                "SSX3_RESOLUTION" => Some(&["100%", "75%", "50%"]),
                "SSX3_LOD_BIAS" => Some(&["Soft", "Standard", "Sharp", "Sharper"]),
                "SSX3_CATCHUP" => Some(&["On", "Off"]),
                "SSX3_RENDER" => Some(&["Native (wgpu)", "Emulated GS"]),
                "SSX3_SND" => Some(&["Native mixer", "Emulated IOP"]),
                "SSX3_IOP_SIO2" => Some(&["Native", "Emulated IOP"]),
                "SSX3_NATIVE_FILES" => Some(&["Native", "Emulated CD"]),
                _ => None,
            };
            let choice_labels = labels
                .map(|l| l.iter().map(|s| s.to_string()).collect())
                .unwrap_or_else(|| {
                    choices
                        .iter()
                        .map(|c| if c.is_empty() { "Auto".to_string() } else { c.clone() })
                        .collect()
                });
            FieldDto {
                key: f.key.to_string(),
                label: f.label.to_string(),
                help: f.help.to_string(),
                kind,
                choices,
                choice_labels,
                min,
                max,
                default: f.default.to_string(),
            }
        })
        .collect()
}

fn state_dto(config: &Config, engine: &dyn EngineAdapter) -> StateDto {
    let d = engine.detect();
    let log_path = config.paths.saves.join("launcher.log");
    StateDto {
        version: env!("CARGO_PKG_VERSION").into(),
        config_path: s2x3_core::config_path().display().to_string(),
        engine_id: engine.id().into(),
        engine_name: engine.name().into(),
        engine_root: config.engine.root.display().to_string(),
        iso: config.paths.iso.display().to_string(),
        saves: config.paths.saves.display().to_string(),
        binary: status(&d.engine),
        disc: status(&d.disc),
        notes: d.notes,
        capabilities: caps(&engine.capabilities()),
        schema: fields(engine),
        values: engine.read_settings().unwrap_or_default(),
        buttons: config.ui.buttons.clone(),
        texreplace: config.paths.texreplace.display().to_string(),
        texreplace_present: config.paths.texreplace.is_dir(),
        log_path: log_path.display().to_string(),
    }
}

#[tauri::command]
fn state() -> Result<StateDto, String> {
    let (config, engine) = load().map_err(|e| format!("{e:#}"))?;
    Ok(state_dto(&config, engine.as_ref()))
}

fn validate(engine: &dyn EngineAdapter, values: &BTreeMap<String, String>) -> anyhow::Result<()> {
    for (key, value) in values {
        let Some(field) = engine.settings_schema().iter().find(|f| f.key == key) else {
            anyhow::bail!("unknown setting `{key}`");
        };
        let ok = match field.kind {
            FieldKind::Bool => matches!(value.as_str(), "0" | "1" | ""),
            FieldKind::Integer { min, max } => value
                .parse::<i64>()
                .map(|v| v >= min && v <= max)
                .unwrap_or(false),
            FieldKind::Choice(choices) => choices.contains(&value.as_str()),
            FieldKind::Text => true,
        };
        if !ok {
            anyhow::bail!("invalid value `{value}` for {key}");
        }
    }
    Ok(())
}

#[tauri::command]
fn save_settings(values: BTreeMap<String, String>) -> Result<StateDto, String> {
    let (mut config, engine) = load().map_err(|e| format!("{e:#}"))?;
    validate(engine.as_ref(), &values).map_err(|e| format!("{e:#}"))?;
    engine
        .write_settings(&values)
        .map_err(|e| format!("{e:#}"))?;
    config.settings = values;
    config.save().map_err(|e| format!("{e:#}"))?;
    Ok(state_dto(&config, engine.as_ref()))
}

#[tauri::command]
fn set_paths(engine_root: String, iso: String, saves: String) -> Result<StateDto, String> {
    let (mut config, _) = load().map_err(|e| format!("{e:#}"))?;

    let root = PathBuf::from(&engine_root);
    if !root.is_dir() {
        return Err(format!("engine folder not found: {}", root.display()));
    }
    let iso_path = PathBuf::from(&iso);
    if !iso_path.is_file() {
        return Err(format!("disc image not found: {}", iso_path.display()));
    }
    let saves_path = PathBuf::from(&saves);
    std::fs::create_dir_all(&saves_path)
        .map_err(|e| format!("cannot create {}: {e}", saves_path.display()))?;

    config.engine.root = root;
    config.paths.iso = iso_path;
    config.paths.saves = saves_path;
    config.save().map_err(|e| format!("{e:#}"))?;
    let (config, engine) = load().map_err(|e| format!("{e:#}"))?;
    Ok(state_dto(&config, engine.as_ref()))
}

const ENGINE_URL: &str = "https://github.com/andersfischernielsen/ssx3-decomp.git";

#[tauri::command]
fn set_buttons(value: String) -> Result<StateDto, String> {
    if !matches!(value.as_str(), "ps2" | "xbox") {
        return Err("button set must be ps2 or xbox".into());
    }
    let (mut config, _) = load().map_err(|e| format!("{e:#}"))?;
    config.ui.buttons = value;
    config.save().map_err(|e| format!("{e:#}"))?;
    let (config, engine) = load().map_err(|e| format!("{e:#}"))?;
    Ok(state_dto(&config, engine.as_ref()))
}

#[tauri::command]
fn set_iso(iso: String) -> Result<StateDto, String> {
    let (mut config, engine) = load().map_err(|e| format!("{e:#}"))?;
    let path = PathBuf::from(&iso);
    if !path.is_file() {
        return Err(format!("disc image not found: {}", path.display()));
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "iso" | "bin" | "img") {
        return Err("pick an .iso, .bin or .img disc image".into());
    }
    config.paths.iso = path;
    config.save().map_err(|e| format!("{e:#}"))?;
    let (config, engine) = load().map_err(|e| format!("{e:#}"))?;
    Ok(state_dto(&config, engine.as_ref()))
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("only http and https links".into());
    }
    #[cfg(target_os = "linux")]
    let mut cmd = std::process::Command::new("xdg-open");
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    };
    cmd.arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("open browser: {e}"))
}

#[tauri::command]
fn choose_iso() -> Result<StateDto, String> {
    let path = rfd::FileDialog::new()
        .add_filter("Disc image", &["iso", "bin", "img"])
        .pick_file()
        .ok_or("cancelled")?;
    set_iso(path.display().to_string())
}

#[tauri::command]
fn set_engine_root(root: String) -> Result<StateDto, String> {
    let (mut config, engine) = load().map_err(|e| format!("{e:#}"))?;
    let path = PathBuf::from(&root);
    if !path.is_dir() {
        return Err(format!("folder not found: {}", path.display()));
    }
    config.engine.root = path;
    config.save().map_err(|e| format!("{e:#}"))?;
    let (config, engine) = load().map_err(|e| format!("{e:#}"))?;
    Ok(state_dto(&config, engine.as_ref()))
}

fn shell_command(command: &str) -> std::process::Command {
    #[cfg(target_os = "windows")]
    {
        let mut cmd = std::process::Command::new("powershell");
        cmd.args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            command,
        ]);
        cmd
    }
    #[cfg(not(target_os = "windows"))]
    {
        let mut cmd = std::process::Command::new("bash");
        cmd.args(["-lc", command]);
        cmd
    }
}

fn spawn_build(root: &Path, iso: &Path, command: &str) -> Result<u32, String> {
    std::fs::create_dir_all(root).map_err(|e| format!("create {}: {e}", root.display()))?;
    let log = s2x3_core::Config::load()
        .map_err(|e| format!("{e:#}"))?
        .resolved()
        .paths
        .saves
        .join("install.log");
    if let Some(dir) = log.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let out = std::fs::File::create(&log).map_err(|e| format!("create {}: {e}", log.display()))?;
    let err = out.try_clone().map_err(|e| format!("{e}"))?;
    let mut child = shell_command(command)
        .current_dir(root)
        .env("SSX3_ISO", iso)
        .stdout(out)
        .stderr(err)
        .spawn()
        .map_err(|e| format!("start build: {e}"))?;
    let pid = child.id();
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(pid)
}

#[tauri::command]
fn bootstrap_engine() -> Result<u32, String> {
    let (config, _) = load().map_err(|e| format!("{e:#}"))?;
    let root = config.engine.root.clone();
    let iso = config.paths.iso.clone();
    #[cfg(target_os = "windows")]
    let cmd = format!(
        "if (-not (Test-Path 'tools/setup.py')) {{ git clone --depth 1 {ENGINE_URL} . }}\nif ($LASTEXITCODE -ne 0) {{ exit $LASTEXITCODE }}\npython tools/setup.py\nif ($LASTEXITCODE -ne 0) {{ exit $LASTEXITCODE }}\ncargo build --profile game -p ssx3"
    );
    #[cfg(not(target_os = "windows"))]
    let cmd = format!(
        "set -e; if [ ! -f tools/setup.py ]; then git clone --depth 1 {ENGINE_URL} .; fi; python3 tools/setup.py && cargo build --profile game -p ssx3"
    );
    spawn_build(&root, &iso, &cmd)
}

#[tauri::command]
fn update_engine() -> Result<u32, String> {
    let (config, _) = load().map_err(|e| format!("{e:#}"))?;
    let root = config.engine.root.clone();
    let iso = config.paths.iso.clone();
    #[cfg(target_os = "windows")]
    let command = "git pull --ff-only\nif ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }\npython tools/setup.py\nif ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }\ncargo build --profile game -p ssx3";
    #[cfg(not(target_os = "windows"))]
    let command =
        "set -e; git pull --ff-only; python3 tools/setup.py && cargo build --profile game -p ssx3";
    spawn_build(&root, &iso, command)
}

#[derive(Serialize)]
struct UpdateDto {
    available: bool,
    local: String,
    remote: String,
    detail: String,
}

#[tauri::command]
fn check_engine_update() -> Result<UpdateDto, String> {
    let (config, _) = load().map_err(|e| format!("{e:#}"))?;
    let root = config.engine.root;
    if !root.join(".git").exists() {
        return Ok(UpdateDto {
            available: false,
            local: String::new(),
            remote: String::new(),
            detail: "not a git checkout".into(),
        });
    }
    let local = std::process::Command::new("git")
        .args(["-C"])
        .arg(&root)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| format!("git: {e}"))?;
    let remote = std::process::Command::new("git")
        .args(["-C"])
        .arg(&root)
        .args(["ls-remote", "origin", "HEAD"])
        .output()
        .map_err(|e| format!("git: {e}"))?;
    let local_s = String::from_utf8_lossy(&local.stdout).trim().to_string();
    let remote_line = String::from_utf8_lossy(&remote.stdout);
    let remote_s = remote_line.split_whitespace().next().unwrap_or("").to_string();
    Ok(UpdateDto {
        available: !remote_s.is_empty() && remote_s != local_s,
        detail: if remote_s.is_empty() {
            "cannot reach the repository".into()
        } else if remote_s == local_s {
            "up to date".into()
        } else {
            "a newer engine commit is available".into()
        },
        local: local_s.chars().take(8).collect(),
        remote: remote_s.chars().take(8).collect(),
    })
}

#[tauri::command]
fn install_log_tail(lines: usize) -> Result<String, String> {
    let (config, _) = load().map_err(|e| format!("{e:#}"))?;
    let path = config.paths.saves.join("install.log");
    read_tail(&path, lines.clamp(1, 2000)).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
fn saves_info() -> Result<s2x3_core::SavesInfo, String> {
    let (_config, engine) = load().map_err(|e| format!("{e:#}"))?;
    engine.saves_info().map_err(|e| format!("{e:#}"))
}

#[tauri::command]
fn export_saves(dest: Option<String>) -> Result<String, String> {
    let (_config, engine) = load().map_err(|e| format!("{e:#}"))?;
    let path = match dest {
        Some(p) => std::path::PathBuf::from(p),
        None => rfd::FileDialog::new()
            .set_file_name("ssx3-mc0.ps2")
            .save_file()
            .ok_or("cancelled")?,
    };
    let out = engine.export_saves(&path).map_err(|e| format!("{e:#}"))?;
    Ok(out.display().to_string())
}

#[tauri::command]
fn import_saves(src: Option<String>) -> Result<String, String> {
    let (_config, engine) = load().map_err(|e| format!("{e:#}"))?;
    let path = match src {
        Some(p) => std::path::PathBuf::from(p),
        None => rfd::FileDialog::new()
            .add_filter("PS2 memory card", &["ps2", "bin", "img"])
            .pick_file()
            .ok_or("cancelled")?,
    };
    let dst = engine.import_saves(&path).map_err(|e| format!("{e:#}"))?;
    Ok(dst.display().to_string())
}

#[tauri::command]
fn launch(overrides: Option<BTreeMap<String, String>>) -> Result<u32, String> {
    let (config, engine) = load().map_err(|e| format!("{e:#}"))?;
    let log = config.paths.saves.join("launcher.log");
    let mut child = engine
        .launch(&LaunchConfig {
            overrides: overrides.unwrap_or_default(),
            extra_env: BTreeMap::new(),
            extra_args: vec![],
            log: Some(log),
        })
        .map_err(|e| format!("{e:#}"))?;
    let pid = child.id();
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(pid)
}

#[tauri::command]
fn install() -> Result<u32, String> {
    let (_config, engine) = load().map_err(|e| format!("{e:#}"))?;
    let mut child = engine.install().map_err(|e| format!("{e:#}"))?;
    let pid = child.id();
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(pid)
}

#[tauri::command]
fn log_tail(lines: usize) -> Result<String, String> {
    let (config, _) = load().map_err(|e| format!("{e:#}"))?;
    let path = config.paths.saves.join("launcher.log");
    read_tail(&path, lines.clamp(1, 2000)).map_err(|e| format!("{e:#}"))
}

fn read_tail(path: &Path, lines: usize) -> anyhow::Result<String> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(e) => anyhow::bail!("read {}: {e}", path.display()),
    };
    let all: Vec<&str> = text.lines().collect();
    let start = all.len().saturating_sub(lines);
    Ok(all[start..].join("\n"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.set_icon(tauri::include_image!("icons/icon.png"));
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            state,
            save_settings,
            set_paths,
            set_iso,
            choose_iso,
            set_buttons,
            open_url,
            set_engine_root,
            bootstrap_engine,
            update_engine,
            check_engine_update,
            install_log_tail,
            saves_info,
            export_saves,
            import_saves,
            install,
            launch,
            log_tail
        ])
        .run(tauri::generate_context!())
        .expect("error while running S2X3");
}

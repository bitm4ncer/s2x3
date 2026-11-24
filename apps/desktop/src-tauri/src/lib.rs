use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use slope3_core::{
    Capabilities, Config, EngineAdapter, FieldKind, LaunchConfig, Status,
};
use slope3_engines::by_id;

#[derive(Serialize)]
struct FieldDto {
    key: String,
    label: String,
    help: String,
    kind: String,
    choices: Vec<String>,
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
            FieldDto {
                key: f.key.to_string(),
                label: f.label.to_string(),
                help: f.help.to_string(),
                kind,
                choices,
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
        config_path: slope3_core::config_path().display().to_string(),
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
        .invoke_handler(tauri::generate_handler![
            state,
            save_settings,
            set_paths,
            launch,
            log_tail
        ])
        .run(tauri::generate_context!())
        .expect("error while running Slope3");
}

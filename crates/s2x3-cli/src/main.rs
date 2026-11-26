//! `s2x3` - the headless launcher core.
//!
//! Everything the future UI needs is here first: config, detection,
//! settings and launch. The UI is a thin layer on top of these calls.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use s2x3_core::{config_path, FieldKind, Status};
use s2x3_engines::by_id;

#[derive(Parser)]
#[command(name = "s2x3", version, about = "Launcher for SSX 3 ports")]
struct Cli {
    /// Use another config file (also S2X3_CONFIG).
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum SavesAction {
    /// Where the save file is and how big it is.
    Info,
    /// Copy the save file to a path (file or folder).
    Export { path: PathBuf },
    /// Replace the save file with a copy from a path (keeps a backup).
    Import { path: PathBuf },
}

#[derive(Subcommand)]
enum Command {
    /// Write the launcher config.
    Init {
        /// Engine checkout (the ssx3-decomp repository).
        #[arg(long)]
        engine_root: PathBuf,
        /// Disc image (PS2 USA, SLUS_207.72).
        #[arg(long)]
        iso: Option<PathBuf>,
        /// Where saves, the memory card and the settings file live.
        #[arg(long)]
        saves: Option<PathBuf>,
    },
    /// What is installed and where.
    Detect,
    /// Paths, versions and hints for a bug report.
    Doctor,
    /// List, set or remove engine settings.
    Settings {
        /// KEY=VALUE, repeatable.
        #[arg(long = "set", value_name = "KEY=VALUE")]
        set: Vec<String>,
        /// Remove a setting (back to its default).
        #[arg(long = "unset", value_name = "KEY")]
        unset: Vec<String>,
    },
    /// Inspect, export or import the engine's save file.
    Saves {
        #[command(subcommand)]
        action: SavesAction,
    },
    /// Build the engine from its sources (setup + cargo build).
    Install,
    /// Start the engine.
    Launch {
        /// KEY=VALUE for this run only, repeatable.
        #[arg(long = "set", value_name = "KEY=VALUE")]
        set: Vec<String>,
        /// Raw KEY=VALUE environment for advanced keys outside the schema.
        #[arg(long = "env", value_name = "KEY=VALUE")]
        env: Vec<String>,
        /// Passed to the engine unchanged (after `--`).
        #[arg(last = true)]
        args: Vec<String>,
    },
}

fn main() {
    if let Err(e) = run() {
        eprintln!("s2x3: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    if let Some(path) = &cli.config {
        std::env::set_var(s2x3_core::CONFIG_ENV, path);
    }
    let mut config = s2x3_core::Config::load()?;
    config = config.resolved();

    match cli.command {
        Command::Init {
            engine_root,
            iso,
            saves,
        } => {
            config.engine.id = "ps2-rust".into();
            config.engine.root = engine_root
                .canonicalize()
                .with_context(|| format!("engine root {}", engine_root.display()))?;
            if let Some(iso) = iso {
                config.paths.iso = iso
                    .canonicalize()
                    .with_context(|| format!("disc image {}", iso.display()))?;
            }
            if let Some(saves) = saves {
                config.paths.saves = saves;
            }
            config.save()?;
            println!("wrote {}", config_path().display());
            // `init` succeeds even when the engine is not built yet.
            if let Err(e) = detect(&config) {
                eprintln!("(not ready yet: {e})");
            }
        }
        Command::Saves { action } => saves(&config, action)?,
        Command::Install => install(&config)?,
        Command::Detect => detect(&config)?,
        Command::Doctor => doctor(&config)?,
        Command::Settings { set, unset } => settings(&config, &set, &unset)?,
        Command::Launch { set, env, args } => launch(&config, &set, &env, args)?,
    }
    Ok(())
}

fn adapter(config: &s2x3_core::Config) -> Result<Box<dyn s2x3_core::EngineAdapter>> {
    by_id(config)
}

fn saves(config: &s2x3_core::Config, action: SavesAction) -> Result<()> {
    let engine = adapter(config)?;
    match action {
        SavesAction::Info => {
            let info = engine.saves_info()?;
            println!("path:   {}", info.path.display());
            println!("exists: {}", info.exists);
            if info.exists {
                println!("size:   {}", s2x3_core::human_size(info.size));
            }
        }
        SavesAction::Export { path } => {
            let out = engine.export_saves(&path)?;
            println!("exported to {}", out.display());
        }
        SavesAction::Import { path } => {
            let dst = engine.import_saves(&path)?;
            println!("imported into {}", dst.display());
        }
    }
    Ok(())
}

fn install(config: &s2x3_core::Config) -> Result<()> {
    let engine = adapter(config)?;
    let mut child = engine.install()?;
    println!(
        "building engine, pid {} (log: {})",
        child.id(),
        config.paths.saves.join("install.log").display()
    );
    let status = child.wait()?;
    println!("build exited: {status}");
    if !status.success() {
        bail!(
            "engine build failed; see {}",
            config.paths.saves.join("install.log").display()
        );
    }
    Ok(())
}

fn detect(config: &s2x3_core::Config) -> Result<()> {
    let engine = adapter(config)?;
    let d = engine.detect();
    println!("engine: {} ({})", engine.name(), engine.id());
    println!("  root: {}", config.engine.root.display());
    match &d.engine {
        Status::Ready(s) => println!("  binary: ok   {s}"),
        Status::Missing(s) => println!("  binary: MISSING {s}"),
    }
    match &d.disc {
        Status::Ready(s) => println!("  disc: ok     {s}"),
        Status::Missing(s) => println!("  disc: MISSING {s}"),
    }
    for note in &d.notes {
        println!("  note: {note}");
    }
    if !(d.engine.is_ready() && d.disc.is_ready()) {
        bail!("not ready");
    }
    Ok(())
}

fn doctor(config: &s2x3_core::Config) -> Result<()> {
    println!("s2x3 {}", env!("CARGO_PKG_VERSION"));
    println!("config: {}", config_path().display());
    println!("os: {} {}", std::env::consts::OS, std::env::consts::ARCH);
    let engine = adapter(config)?;
    let caps = engine.capabilities();
    println!(
        "capabilities: platforms={:?} plugins={} netplay={} splitscreen={} max_tick_hz={}",
        caps.platforms, caps.plugins, caps.netplay, caps.splitscreen, caps.max_tick_hz
    );
    for note in &caps.notes {
        println!("  {note}");
    }
    let d = engine.detect();
    println!("engine: {:?}", d.engine);
    println!("disc:   {:?}", d.disc);
    for note in &d.notes {
        println!("  {note}");
    }
    Ok(())
}

fn parse_pairs(pairs: &[String]) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for pair in pairs {
        let (k, v) = pair
            .split_once('=')
            .with_context(|| format!("expected KEY=VALUE, got `{pair}`"))?;
        out.insert(k.trim().to_string(), v.trim().to_string());
    }
    Ok(out)
}

fn validate(
    engine: &dyn s2x3_core::EngineAdapter,
    values: &BTreeMap<String, String>,
) -> Result<()> {
    for (key, value) in values {
        let Some(field) = engine.settings_schema().iter().find(|f| f.key == key) else {
            let known: Vec<_> = engine.settings_schema().iter().map(|f| f.key).collect();
            bail!("unknown setting `{key}` (known: {})", known.join(", "));
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
            bail!("invalid value `{value}` for {key}");
        }
    }
    Ok(())
}

fn settings(config: &s2x3_core::Config, set: &[String], unset: &[String]) -> Result<()> {
    let engine = adapter(config)?;
    let pairs = parse_pairs(set)?;
    validate(engine.as_ref(), &pairs)?;

    if !pairs.is_empty() || !unset.is_empty() {
        let mut values = engine.read_settings()?;
        for (k, v) in pairs {
            if v.is_empty() {
                values.remove(&k);
            } else {
                values.insert(k, v);
            }
        }
        for k in unset {
            values.remove(k);
        }
        engine.write_settings(&values)?;
        println!(
            "wrote {}",
            config.paths.saves.join("ssx3-settings.cfg").display()
        );
    } else {
        let current = engine.read_settings()?;
        for field in engine.settings_schema() {
            let value = current
                .get(field.key)
                .map(String::as_str)
                .unwrap_or(field.default);
            println!("{:<18} {:<22} {}", field.key, value, field.label);
        }
    }
    Ok(())
}

fn launch(
    config: &s2x3_core::Config,
    set: &[String],
    env: &[String],
    args: Vec<String>,
) -> Result<()> {
    let engine = adapter(config)?;
    let overrides = parse_pairs(set)?;
    validate(engine.as_ref(), &overrides)?;
    let mut child = engine.launch(&s2x3_core::LaunchConfig {
        overrides,
        extra_env: parse_pairs(env)?,
        extra_args: args,
        log: None,
    })?;
    println!("running pid {}", child.id());
    let status = child.wait()?;
    println!("engine exited: {status}");
    Ok(())
}

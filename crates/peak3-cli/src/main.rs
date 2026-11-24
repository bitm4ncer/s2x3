//! `peak3` - the headless launcher core.
//!
//! Everything the future UI needs is here first: config, detection,
//! settings and launch. The UI is a thin layer on top of these calls.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use peak3_core::{config_path, FieldKind, Status};
use peak3_engines::by_id;

#[derive(Parser)]
#[command(name = "peak3", version, about = "Launcher for SSX 3 ports")]
struct Cli {
    /// Use another config file (also PEAK3_CONFIG).
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
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
    /// Start the engine.
    Launch {
        /// KEY=VALUE for this run only, repeatable.
        #[arg(long = "set", value_name = "KEY=VALUE")]
        set: Vec<String>,
        /// Passed to the engine unchanged (after `--`).
        #[arg(last = true)]
        args: Vec<String>,
    },
}

fn main() {
    if let Err(e) = run() {
        eprintln!("peak3: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    if let Some(path) = &cli.config {
        std::env::set_var(peak3_core::CONFIG_ENV, path);
    }
    let mut config = peak3_core::Config::load()?;
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
        Command::Detect => detect(&config)?,
        Command::Doctor => doctor(&config)?,
        Command::Settings { set, unset } => settings(&config, &set, &unset)?,
        Command::Launch { set, args } => launch(&config, &set, args)?,
    }
    Ok(())
}

fn adapter(config: &peak3_core::Config) -> Result<Box<dyn peak3_core::EngineAdapter>> {
    by_id(config)
}

fn detect(config: &peak3_core::Config) -> Result<()> {
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

fn doctor(config: &peak3_core::Config) -> Result<()> {
    println!("peak3 {}", env!("CARGO_PKG_VERSION"));
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
    engine: &dyn peak3_core::EngineAdapter,
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

fn settings(config: &peak3_core::Config, set: &[String], unset: &[String]) -> Result<()> {
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
        println!("wrote {}", config.paths.saves.join("ssx3-settings.cfg").display());
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

fn launch(config: &peak3_core::Config, set: &[String], args: Vec<String>) -> Result<()> {
    let engine = adapter(config)?;
    let overrides = parse_pairs(set)?;
    validate(engine.as_ref(), &overrides)?;
    let mut child = engine.launch(&peak3_core::LaunchConfig {
        overrides,
        extra_args: args,
    })?;
    println!("running pid {}", child.id());
    let status = child.wait()?;
    println!("engine exited: {status}");
    Ok(())
}

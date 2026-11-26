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
enum TexturesAction {
    /// List the installed packs and their state.
    List,
    /// Install a pack from a zip or folder.
    Import { path: PathBuf },
    /// Delete an installed pack.
    Remove { id: String },
    /// Turn a pack on (rebuilds the active set).
    Enable { id: String },
    /// Turn a pack off (rebuilds the active set).
    Disable { id: String },
    /// Extract the disc's textures offline (no game start).
    Extract,
    /// Upscale the extracted textures into an HD pack (needs a Vulkan GPU).
    Upscale,
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
    /// Install, manage and build texture packs.
    Textures {
        #[command(subcommand)]
        action: TexturesAction,
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

/// The `textures` subcommand: thin CLI over `s2x3_core::textures`. Mutating
/// actions rebuild the active set and save the config.
fn textures_cmd(config: &mut s2x3_core::Config, action: &TexturesAction) -> Result<()> {
    use s2x3_core::textures as tx;
    match action {
        TexturesAction::List => {
            let entries = tx::list(&config.paths.textures_packs());
            if entries.is_empty() {
                println!("no texture packs installed");
            }
            for e in entries {
                let state = match config.textures.enabled.get(&e.id) {
                    Some(true) => "on",
                    Some(false) => "off",
                    None => "-",
                };
                let kind = e
                    .pack
                    .as_ref()
                    .map(|p| p.manifest.kind.to_string())
                    .unwrap_or_else(|| "?".into());
                println!(
                    "{:>4}  {:<20} {:<9} {:>4}",
                    state, e.id, kind, e.texture_count
                );
                if let Some(err) = &e.error {
                    println!("       ! {err}");
                }
            }
        }
        TexturesAction::Import { path } => {
            let id = tx::import(path, &config.paths.textures_packs(), false)?;
            textures_apply(config)?;
            println!("installed: {id}");
        }
        TexturesAction::Remove { id } => {
            tx::remove(&config.paths.textures_packs(), id)?;
            config.textures.enabled.remove(id);
            config.textures.priority.retain(|p| p != id);
            textures_apply(config)?;
            println!("removed: {id}");
        }
        TexturesAction::Enable { id } => {
            textures_ensure_installed(config, id)?;
            config.textures.enabled.insert(id.clone(), true);
            textures_apply(config)?;
            println!("enabled: {id}");
        }
        TexturesAction::Disable { id } => {
            config.textures.enabled.insert(id.clone(), false);
            textures_apply(config)?;
            println!("disabled: {id}");
        }
        TexturesAction::Extract => {
            let summary = tx::run_extract(config)?;
            println!("{summary}");
        }
        TexturesAction::Upscale => {
            let pack = tx::run_upscale(config)?;
            textures_apply(config)?;
            println!("pack created: {}", pack.display());
        }
    }
    config.save()?;
    Ok(())
}

/// Rebuild the active merge folder from the config state.
fn textures_apply(config: &s2x3_core::Config) -> Result<()> {
    use s2x3_core::textures as tx;
    let report = tx::merge(
        &config.paths.textures_packs(),
        &config.paths.textures_active(),
        &config.textures.enabled,
        &config.textures.priority,
    )?;
    if !report.conflicts.is_empty() {
        eprintln!(
            "note: several active packs replace the same textures: {}",
            report.conflicts.join(", ")
        );
    }
    Ok(())
}

fn textures_ensure_installed(config: &s2x3_core::Config, id: &str) -> Result<()> {
    use s2x3_core::textures as tx;
    if !tx::list(&config.paths.textures_packs())
        .iter()
        .any(|e| e.id == id && e.pack.is_some())
    {
        bail!("pack `{id}` is not installed");
    }
    Ok(())
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
        Command::Textures { action } => {
            textures_cmd(&mut config, &action)?;
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use s2x3_core::textures as tx;

    const MANIFEST: &str = "\
name = \"Xbox Buttons\"
author = \"s2x3\"
version = \"1.0.0\"
kind = \"own-art\"
";

    #[test]
    fn textures_actions_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var(s2x3_core::CONFIG_ENV, tmp.path().join("config.toml"));

        // install a pack from a folder, via the CLI surface
        let src = tmp.path().join("Xbox Buttons");
        std::fs::create_dir_all(src.join("textures")).unwrap();
        std::fs::write(src.join(tx::MANIFEST), MANIFEST).unwrap();
        std::fs::write(src.join("textures").join("f897162c0f661c8b.png"), b"png").unwrap();

        let mut config = s2x3_core::Config::load().unwrap().resolved();
        textures_cmd(&mut config, &TexturesAction::Import { path: src.clone() }).unwrap();

        textures_cmd(
            &mut config,
            &TexturesAction::Enable {
                id: "xbox-buttons".into(),
            },
        )
        .unwrap();
        assert_eq!(
            config.textures.enabled.get("xbox-buttons"),
            Some(&true),
            "enable writes the config state"
        );
        assert!(config
            .paths
            .textures_active()
            .join("f897162c0f661c8b.png")
            .is_file());

        // unknown id: readable error, config unchanged
        let err = format!(
            "{:#}",
            textures_cmd(&mut config, &TexturesAction::Enable { id: "ghost".into() },).unwrap_err()
        );
        assert!(err.contains("ghost"), "got: {err}");

        textures_cmd(
            &mut config,
            &TexturesAction::Disable {
                id: "xbox-buttons".into(),
            },
        )
        .unwrap();
        assert_eq!(config.textures.enabled.get("xbox-buttons"), Some(&false));
        assert!(!config
            .paths
            .textures_active()
            .join("f897162c0f661c8b.png")
            .is_file());

        textures_cmd(
            &mut config,
            &TexturesAction::Remove {
                id: "xbox-buttons".into(),
            },
        )
        .unwrap();
        assert!(!config.paths.textures_packs().join("xbox-buttons").exists());
    }

    #[test]
    fn extract_and_upscale_fail_readable_without_tooling() {
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var(s2x3_core::CONFIG_ENV, tmp.path().join("config.toml"));
        let mut config = s2x3_core::Config::load().unwrap().resolved();
        config.engine.root = tmp.path().join("no-engine");

        let err = format!(
            "{:#}",
            textures_cmd(&mut config, &TexturesAction::Extract).unwrap_err()
        );
        assert!(
            err.contains("dump-textures") && err.contains("engine"),
            "got: {err}"
        );
        let err = format!(
            "{:#}",
            textures_cmd(&mut config, &TexturesAction::Upscale).unwrap_err()
        );
        assert!(
            err.contains("upscale-textures") || err.contains("python"),
            "got: {err}"
        );
    }
}

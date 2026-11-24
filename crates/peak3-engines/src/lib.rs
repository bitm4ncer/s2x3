//! Engine adapters. One module per engine; [`by_id`] is the registry.

mod ps2;

pub use ps2::Ps2Rust;

use peak3_core::{Config, EngineAdapter};

/// Build the adapter named in the config. Unknown ids are an error, so a
/// typo never silently launches the wrong engine.
pub fn by_id(config: &Config) -> anyhow::Result<Box<dyn EngineAdapter>> {
    match config.engine.id.as_str() {
        ps2::ID => Ok(Box::new(Ps2Rust::new(config.clone().resolved()))),
        other => anyhow::bail!("unknown engine `{other}` (known: {})", ps2::ID),
    }
}

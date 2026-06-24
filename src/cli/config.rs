use crate::config::StyxConfig;

/// Generate a fresh config template at ~/.config/styx/config.toml.
pub fn run_init_config() -> anyhow::Result<()> {
    let path = StyxConfig::init_template()?;
    println!("Config written to {}", path.display());
    Ok(())
}

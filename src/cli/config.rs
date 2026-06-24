use crate::config::ClioConfig;

/// Generate a fresh config template at ~/.config/clio/config.toml.
///
/// If a config file already exists at that path, the command prints a message
/// and exits without overwriting it.
pub fn run_init_config() -> anyhow::Result<()> {
    let path = ClioConfig::config_path()?;

    if path.exists() {
        println!(
            "Config already exists at {} — not overwriting.",
            path.display()
        );
        return Ok(());
    }

    let path = ClioConfig::init_template()?;
    println!("Config written to {}", path.display());
    Ok(())
}

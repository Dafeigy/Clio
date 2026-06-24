use anyhow::Context;
use serde::Deserialize;
use std::path::PathBuf;

/// Top-level styx configuration, loaded from a TOML file.
#[derive(Deserialize, Default)]
pub struct StyxConfig {
    #[serde(default)]
    pub s3: S3FileConfig,
}

/// The `[s3]` section of the config file.
///
/// All fields are optional — empty strings are treated as "not set",
/// and the caller layers env vars and defaults on top.
#[derive(Deserialize, Default)]
pub struct S3FileConfig {
    pub endpoint: Option<String>,
    pub bucket: Option<String>,
    pub prefix: Option<String>,
    pub region: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
}

impl S3FileConfig {
    /// Treat empty strings as `None` so `bucket = ""` in the config file
    /// behaves the same as omitting the key entirely.
    fn normalize(&mut self) {
        for field in [
            &mut self.endpoint,
            &mut self.bucket,
            &mut self.prefix,
            &mut self.region,
            &mut self.access_key,
            &mut self.secret_key,
        ] {
            if field.as_deref() == Some("") {
                *field = None;
            }
        }
    }
}

/// Auto-generated template — optional fields have their defaults pre-filled,
/// required fields are left empty for the user to fill in.
const CONFIG_TEMPLATE: &str = r#"# styx configuration
#
# Environment variables (STYX_S3_*) take precedence over values set here.
# Fill in the required fields below, then run `styx sync-status` to verify.

[s3]
# S3-compatible endpoint (default shown)
endpoint = "https://s3.amazonaws.com"

# Bucket name (required)
bucket = ""

# Key prefix inside the bucket
prefix = "styx/"

# AWS / S3 region
region = "us-east-1"

# Access credentials (required)
access_key = ""
secret_key = ""
"#;

impl StyxConfig {
    /// Resolve the path to the config file.
    ///
    /// Respects `STYX_CONFIG` env var for custom locations.
    /// Falls back to the XDG config directory: `~/.config/styx/config.toml`.
    pub fn config_path() -> anyhow::Result<PathBuf> {
        if let Ok(custom) = std::env::var("STYX_CONFIG") {
            return Ok(PathBuf::from(custom));
        }

        let dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("could not determine config directory"))?
            .join("styx");

        Ok(dir.join("config.toml"))
    }

    /// Load config from the config file.
    ///
    /// If the config file does not exist, a template is auto-generated so the
    /// user knows where to put their credentials.  Required fields will be
    /// empty, so the first sync operation will fail with a helpful pointer.
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::config_path()?;

        if !path.exists() {
            Self::write_template(&path)?;
            eprintln!(
                "📝 Config template created at {}",
                path.display()
            );
            eprintln!("   Edit it with your S3 credentials, then try again.\n");
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read config at {}", path.display()))?;

        let mut config: Self = toml::from_str(&content)
            .with_context(|| format!("failed to parse config at {}", path.display()))?;

        config.s3.normalize();
        Ok(config)
    }

    /// Write a fresh template to the config path (always overwrites).
    ///
    /// Used by `styx init-config`.
    pub fn init_template() -> anyhow::Result<PathBuf> {
        let path = Self::config_path()?;
        Self::write_template(&path)?;
        Ok(path)
    }

    /// Internal helper — create parent dirs and write the template.
    fn write_template(path: &std::path::Path) -> anyhow::Result<()> {
        let dir = path.parent().unwrap();
        std::fs::create_dir_all(dir)
            .with_context(|| format!("failed to create config directory {}", dir.display()))?;

        std::fs::write(path, CONFIG_TEMPLATE)
            .with_context(|| format!("failed to write config template to {}", path.display()))?;

        Ok(())
    }
}

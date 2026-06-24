use crate::sync::manifest::SyncManifest;
use anyhow::Context;
use s3::creds::Credentials;
use s3::{Bucket, Region};
use std::path::Path;

/// Configuration for the S3 backend.
///
/// Values are resolved with this priority (highest first):
/// 1. `STYX_S3_*` environment variables
/// 2. Config file (`~/.config/styx/config.toml`)
/// 3. Built-in defaults (endpoint, prefix, region)
pub struct S3Config {
    pub endpoint: String,
    pub bucket_name: String,
    pub prefix: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
}

impl S3Config {
    /// Load config: config file first, then override with any set env vars.
    pub fn load() -> anyhow::Result<Self> {
        let file_cfg = crate::config::StyxConfig::load()?;

        // Each value: env var → config file → hard-coded default
        let endpoint = resolve(
            "STYX_S3_ENDPOINT",
            file_cfg.s3.endpoint.as_deref(),
            "https://s3.amazonaws.com",
        );

        let bucket_name = resolve_required(
            "STYX_S3_BUCKET",
            file_cfg.s3.bucket.as_deref(),
            "S3 bucket is required. Set it in ~/.config/styx/config.toml ([s3] bucket) or the STYX_S3_BUCKET env var.",
        )?;

        let prefix = resolve(
            "STYX_S3_PREFIX",
            file_cfg.s3.prefix.as_deref(),
            "styx/",
        );

        let region = resolve(
            "STYX_S3_REGION",
            file_cfg.s3.region.as_deref(),
            "us-east-1",
        );

        let access_key = resolve_required(
            "STYX_S3_ACCESS_KEY",
            file_cfg.s3.access_key.as_deref(),
            "S3 access key is required. Set it in ~/.config/styx/config.toml ([s3] access_key) or the STYX_S3_ACCESS_KEY env var.",
        )?;

        let secret_key = resolve_required(
            "STYX_S3_SECRET_KEY",
            file_cfg.s3.secret_key.as_deref(),
            "S3 secret key is required. Set it in ~/.config/styx/config.toml ([s3] secret_key) or the STYX_S3_SECRET_KEY env var.",
        )?;

        Ok(Self { endpoint, bucket_name, prefix, region, access_key, secret_key })
    }
}

/// Pick the first of: env var → config file value → default.
fn resolve(env_key: &str, file_val: Option<&str>, default: &str) -> String {
    std::env::var(env_key)
        .ok()
        .or_else(|| file_val.map(String::from))
        .unwrap_or_else(|| default.to_string())
}

/// Like `resolve`, but returns an error when no value is found.
fn resolve_required(env_key: &str, file_val: Option<&str>, hint: &str) -> anyhow::Result<String> {
    std::env::var(env_key)
        .ok()
        .or_else(|| file_val.map(String::from))
        .with_context(|| hint.to_string())
}

/// An S3-compatible storage backend.
pub struct S3Backend {
    bucket: Bucket,
    prefix: String,
}

impl S3Backend {
    /// Create a new S3 backend from config file + env overrides.
    pub fn load() -> anyhow::Result<Self> {
        let config = S3Config::load()?;

        let region = Region::Custom {
            region: config.region,
            endpoint: config.endpoint,
        };

        let credentials = Credentials::new(
            Some(&config.access_key),
            Some(&config.secret_key),
            None,
            None,
            None,
        )
        .map_err(|e| anyhow::anyhow!("S3 credentials error: {}", e))?;

        // Ensure prefix ends with /
        let prefix = if config.prefix.ends_with('/') {
            config.prefix
        } else {
            format!("{}/", config.prefix)
        };

        let bucket =
            Bucket::new(&config.bucket_name, region, credentials)
                .map_err(|e| anyhow::anyhow!("S3 bucket error: {}", e))?;

        Ok(Self { bucket, prefix })
    }

    /// Key for the manifest file in S3.
    fn manifest_key(&self) -> String {
        format!("{}manifest.json", self.prefix)
    }

    /// Key for a database file in S3.
    fn db_key(&self, name: &str) -> String {
        format!("{}{}.redb", self.prefix, name)
    }

    /// Fetch the remote sync manifest.
    pub async fn get_manifest(&self) -> anyhow::Result<Option<SyncManifest>> {
        let key = self.manifest_key();

        match self.bucket.get_object(&key).await {
            Ok(data) => {
                let bytes: Vec<u8> = data.to_vec();
                let manifest: SyncManifest = serde_json::from_slice(&bytes)
                    .with_context(|| "failed to parse remote manifest")?;
                Ok(Some(manifest))
            }
            Err(e) => {
                let err_str = format!("{}", e);
                if err_str.contains("404") || err_str.contains("NoSuchKey") {
                    // No manifest yet — first sync.
                    Ok(None)
                } else {
                    Err(anyhow::anyhow!("S3 error fetching manifest: {}", e))
                }
            }
        }
    }

    /// Upload the sync manifest to S3.
    pub async fn put_manifest(&self, manifest: &SyncManifest) -> anyhow::Result<()> {
        let key = self.manifest_key();
        let data = serde_json::to_vec_pretty(manifest)?;

        self.bucket
            .put_object(&key, &data)
            .await
            .map_err(|e| anyhow::anyhow!("S3 error uploading manifest: {}", e))?;
        Ok(())
    }

    /// Download a database file from S3 to a local path.
    pub async fn download_db(&self, name: &str, dest: &Path) -> anyhow::Result<()> {
        let key = self.db_key(name);

        let data = self
            .bucket
            .get_object(&key)
            .await
            .map_err(|e| anyhow::anyhow!("S3 error downloading @{}: {}", name, e))?;
        let bytes: Vec<u8> = data.to_vec();

        std::fs::write(dest, &bytes)
            .with_context(|| format!("failed to write downloaded database to {}", dest.display()))?;

        Ok(())
    }

    /// Upload a local database file to S3.
    pub async fn upload_db(&self, name: &str, source: &Path) -> anyhow::Result<()> {
        let key = self.db_key(name);
        let data = std::fs::read(source)
            .with_context(|| format!("failed to read local database {}", source.display()))?;

        self.bucket
            .put_object(&key, &data)
            .await
            .map_err(|e| anyhow::anyhow!("S3 error uploading @{}: {}", name, e))?;

        Ok(())
    }
}

use clap::Parser;

/// Command line arguments
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct CliArgs {
    /// Path to config file (default: config.toml)
    #[arg(long, default_value = "config.toml")]
    config: String,

    /// Local address to bind the proxy server, e.g. 0.0.0.0:8080
    #[arg(long)]
    bind_address: Option<String>,

    /// Upstream RPC endpoint to relay proxy requests to, e.g. http://validium-sequencer:8545
    #[arg(long)]
    upstream_url: Option<String>,
}

/// Structure of the config file
#[derive(Debug, serde::Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    #[serde(default = "default_upstream_url")]
    pub upstream_url: String,
    pub admin_keys: Vec<String>,
    pub jwt_expiry_secs: usize,
    pub default_kid: String,
    pub jwt_signer_keys: Vec<super::auth::JwtSignerKeyConfig>,
}

/// Default bind address if not specified anywhere
fn default_bind_address() -> String {
    "0.0.0.0:8080".to_owned()
}

/// Default upstream URL if not specified anywhere
fn default_upstream_url() -> String {
    "http://validium-sequencer:8545".to_owned()
}

/// Load configuration from CLI, config file, and defaults
pub fn load_config() -> anyhow::Result<AppConfig> {
    let args = CliArgs::parse();

    let mut cfg: AppConfig = config::Config::builder()
        .add_source(config::File::from(args.config.as_ref()).required(true))
        .build()?
        .try_deserialize()?;

    // Override config with CLI arguments if provided
    if let Some(val) = args.bind_address {
        cfg.bind_address = val;
    }
    if let Some(val) = args.upstream_url {
        cfg.upstream_url = val;
    }

    // Validate bind_address format
    cfg.bind_address
        .parse::<std::net::SocketAddr>()
        .map_err(|_| {
            anyhow::anyhow!(
                "Invalid bind_address: {}. Expected format like 0.0.0.0:8080",
                cfg.bind_address
            )
        })?;

    // Validate upstream_url format
    if !cfg.upstream_url.starts_with("http://") && !cfg.upstream_url.starts_with("https://") {
        anyhow::bail!(
            "Invalid upstream_url: {}. Must start with http:// or https://",
            cfg.upstream_url
        );
    }

    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::{Config, File, FileFormat};

    #[test]
    fn test_app_config_parse_from_toml() {
        // Example TOML configuration string
        let toml = r#"
            bind_address = "127.0.0.1:12345"
            upstream_url = "http://example.com:8545"
            admin_keys = [
              "admin-token-1-abcdefg",
              "admin-token-2-hijklmn"
            ]
            jwt_expiry_secs = 3600
            default_kid = "key-2025-07"

            jwt_signer_keys = [
              { kid = "key-2025-07", secret = "supersecret1" },
              { kid = "key-2025-06", secret = "supersecret2" }
            ]
        "#;

        // Parse config from string
        let cfg: AppConfig = Config::builder()
            .add_source(File::from_str(toml, FileFormat::Toml))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();

        // Check values
        assert_eq!(cfg.bind_address, "127.0.0.1:12345");
        assert_eq!(cfg.upstream_url, "http://example.com:8545");
        assert_eq!(
            cfg.admin_keys,
            vec![
                "admin-token-1-abcdefg".to_string(),
                "admin-token-2-hijklmn".to_string()
            ]
        );
        assert_eq!(cfg.jwt_expiry_secs, 3600);
        assert_eq!(cfg.default_kid, "key-2025-07".to_string());
        assert_eq!(cfg.jwt_signer_keys.len(), 2);
        assert_eq!(cfg.jwt_signer_keys[0].kid, "key-2025-07");
        assert_eq!(cfg.jwt_signer_keys[0].secret, "supersecret1");
        assert_eq!(cfg.jwt_signer_keys[1].kid, "key-2025-06");
        assert_eq!(cfg.jwt_signer_keys[1].secret, "supersecret2");
    }
}

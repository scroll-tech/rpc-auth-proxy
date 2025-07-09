mod auth;
mod proxy;

use std::io::{stdout, Write};
use std::net::SocketAddr;

use dashmap::DashSet;
use jsonrpsee::{Methods, server::Server};
use tower::ServiceBuilder;
use tower_http::auth::AsyncRequireAuthorizationLayer;
use auth::{AuthenticationMiddleware, JwtSigner, SiweAuthRpcImpl, SiweAuthRpcServer};
use proxy::{EthRpcProxyImpl, EthRpcProxyServer};

use clap::Parser;
use config;

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
struct AppConfig {
    #[serde(default = "default_bind_address")]
    bind_address: String,
    #[serde(default = "default_upstream_url")]
    upstream_url: String,
    // L2 RPC URL for smart contract signature verification
    #[serde(default = "default_l2_rpc_url")]
    l2_rpc_url: String,
    admin_keys: Vec<String>,
    jwt_expiry_secs: usize,
    default_kid: String,
    jwt_signer_keys: Vec<auth::JwtSignerKeyConfig>,
}

// Default L2 RPC URL
fn default_l2_rpc_url() -> String {
    "http://localhost:8545".to_string()
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
fn load_config() -> anyhow::Result<AppConfig> {
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
    cfg.bind_address.parse::<std::net::SocketAddr>()
        .map_err(|_| anyhow::anyhow!("Invalid bind_address: {}. Expected format like 0.0.0.0:8080", cfg.bind_address))?;

    // Validate upstream_url format
    if !cfg.upstream_url.starts_with("http://") && !cfg.upstream_url.starts_with("https://") {
        anyhow::bail!("Invalid upstream_url: {}. Must start with http:// or https://", cfg.upstream_url);
    }

    Ok(cfg)
}

fn all_apis(jwt: JwtSigner, jwt_expiry_secs: usize, upstream_url: &str, l2_rpc_url: &str) -> anyhow::Result<impl Into<Methods>> {
    let auth_server = SiweAuthRpcImpl::new(jwt, jwt_expiry_secs, l2_rpc_url)?;
    let proxy_server = EthRpcProxyImpl::new(upstream_url)?;
    let mut methods = auth_server.into_rpc();
    methods.merge(proxy_server.into_rpc())?;
    Ok(methods)
}

pub async fn run_server() -> anyhow::Result<SocketAddr> {
    let cfg = load_config()?;

    let jwt = JwtSigner::from_config(cfg.jwt_signer_keys.as_slice(), &cfg.default_kid)?;

    // Only load admin_keys from config file
    let admin_keys = DashSet::default();
    for key in cfg.admin_keys {
        admin_keys.insert(key);
    }

    let http_middleware = ServiceBuilder::new().layer(AsyncRequireAuthorizationLayer::new(
        AuthenticationMiddleware::new(jwt.clone(), admin_keys),
    ));

    let server = Server::builder()
        .set_http_middleware(http_middleware)
        .build(cfg.bind_address.parse::<SocketAddr>()?)
        .await?;

    let addr = server.local_addr()?;
    println!("Server is listening on {}", addr);
    println!("Upstream endpoint is {}", cfg.upstream_url);

    // not sure why this is needed, but running in a linux/amd64
    // Docker container without this exits immediately.
    stdout().flush().unwrap();

    let handle = server.start(all_apis(jwt, cfg.jwt_expiry_secs, &cfg.upstream_url, &cfg.l2_rpc_url)?);
    handle.stopped().await;
    Ok(addr)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run_server().await?;
    Ok(())
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
        assert_eq!(cfg.admin_keys, vec![
            "admin-token-1-abcdefg".to_string(),
            "admin-token-2-hijklmn".to_string()
        ]);
        assert_eq!(cfg.jwt_expiry_secs, 3600);
        assert_eq!(cfg.default_kid, "key-2025-07".to_string());
        assert_eq!(cfg.jwt_signer_keys.len(), 2);
        assert_eq!(cfg.jwt_signer_keys[0].kid, "key-2025-07");
        assert_eq!(cfg.jwt_signer_keys[0].secret, "supersecret1");
        assert_eq!(cfg.jwt_signer_keys[1].kid, "key-2025-06");
        assert_eq!(cfg.jwt_signer_keys[1].secret, "supersecret2");
    }
}

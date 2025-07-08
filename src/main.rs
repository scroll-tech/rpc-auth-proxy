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
use serde::Deserialize;
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
#[derive(Debug, Deserialize)]
struct AppConfig {
    #[serde(default = "default_bind_address")]
    bind_address: String,
    #[serde(default = "default_upstream_url")]
    upstream_url: String,
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
fn load_config() -> anyhow::Result<(String, String)> {
    let args = CliArgs::parse();

    // Load config.toml
    let mut cfg: AppConfig = config::Config::builder()
        .add_source(config::File::from(args.config.as_ref()).required(false))
        .build()?
        .try_deserialize()?;

    // Override with CLI arguments
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

    Ok((cfg.bind_address, cfg.upstream_url))
}

fn all_apis(jwt: JwtSigner, upstream_url: &str) -> anyhow::Result<impl Into<Methods>> {
    let auth_server = SiweAuthRpcImpl::new(jwt);
    let proxy_server = EthRpcProxyImpl::new(upstream_url)?;
    let mut methods = auth_server.into_rpc();
    methods.merge(proxy_server.into_rpc())?;
    Ok(methods)
}

pub async fn run_server() -> anyhow::Result<SocketAddr> {
    let (bind_address, upstream_url) = load_config()?;

    let jwt = JwtSigner::new();

    let admin_keys = DashSet::default();
    admin_keys.insert("example_admin_token".to_owned());

    let http_middleware = ServiceBuilder::new().layer(AsyncRequireAuthorizationLayer::new(
        AuthenticationMiddleware::new(jwt.clone(), admin_keys),
    ));

    let server = Server::builder()
        .set_http_middleware(http_middleware)
        .build(bind_address.parse::<SocketAddr>()?)
        .await?;

    let addr = server.local_addr()?;
    println!("Server is listening on {}", addr);
    println!("Upstream endpoint is {}", upstream_url);

    // not sure why this is needed, but running in a linux/amd64
    // Docker container without this exits immediately.
    stdout().flush().unwrap();

    let handle = server.start(all_apis(jwt, &upstream_url)?);
    handle.stopped().await;
    Ok(addr)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run_server().await?;
    Ok(())
}

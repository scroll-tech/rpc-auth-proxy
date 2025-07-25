mod auth;
mod config;
mod proxy;

use std::net::SocketAddr;

use auth::{AuthenticationMiddleware, JwtSigner, SiweAuthRpcImpl, SiweAuthRpcServer};
use dashmap::DashSet;
use jsonrpsee::{Methods, server::Server};
use proxy::{EthRpcProxyImpl, EthRpcProxyServer};
use tower::ServiceBuilder;
use tower_http::auth::AsyncRequireAuthorizationLayer;

async fn all_apis(
    jwt: JwtSigner,
    jwt_expiry_secs: usize,
    upstream_url: &str,
) -> anyhow::Result<impl Into<Methods>> {
    let auth_server = SiweAuthRpcImpl::new(jwt, jwt_expiry_secs, upstream_url).await?;
    let proxy_server = EthRpcProxyImpl::new(upstream_url)?;
    let mut methods = auth_server.into_rpc();
    methods.merge(proxy_server.into_rpc())?;
    Ok(methods)
}

pub async fn run_server() -> anyhow::Result<SocketAddr> {
    let cfg = config::load_config()?;
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
    eprintln!("Server is listening on {addr}");
    eprintln!("Upstream endpoint is {}", cfg.upstream_url);

    let methods = all_apis(jwt, cfg.jwt_expiry_secs, &cfg.upstream_url).await?;
    let handle = server.start(methods);
    handle.stopped().await;
    Ok(addr)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match run_server().await {
        Ok(_) => Ok(()),
        Err(err) => {
            eprintln!("Error starting server: {err}");
            std::process::exit(1);
        }
    }
}

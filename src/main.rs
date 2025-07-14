mod auth;
mod config;
mod proxy;

use std::io::{Write, stdout};
use std::net::SocketAddr;

use auth::{AuthenticationMiddleware, JwtSigner, SiweAuthRpcImpl, SiweAuthRpcServer};
use jsonrpsee::{Methods, server::Server};
use proxy::{EthRpcProxyImpl, EthRpcProxyServer};
use tower::ServiceBuilder;
use tower_http::auth::AsyncRequireAuthorizationLayer;

fn all_apis(
    jwt: JwtSigner,
    jwt_expiry_secs: usize,
    upstream_url: &str,
) -> anyhow::Result<impl Into<Methods>> {
    let auth_server = SiweAuthRpcImpl::new(jwt, jwt_expiry_secs);
    let proxy_server = EthRpcProxyImpl::new(upstream_url)?;
    let mut methods = auth_server.into_rpc();
    methods.merge(proxy_server.into_rpc())?;
    Ok(methods)
}

pub async fn run_server() -> anyhow::Result<SocketAddr> {
    let cfg = config::load_config()?;
    let jwt = JwtSigner::from_config(cfg.jwt_signer_keys.as_slice(), &cfg.default_kid)?;

    let http_middleware = ServiceBuilder::new().layer(AsyncRequireAuthorizationLayer::new(
        AuthenticationMiddleware::new(jwt.clone(), cfg.admin_keys.iter().cloned()),
    ));

    let server = Server::builder()
        .set_http_middleware(http_middleware)
        .build(cfg.bind_address.parse::<SocketAddr>()?)
        .await?;

    let addr = server.local_addr()?;
    println!("Server is listening on {addr}");
    println!("Upstream endpoint is {}", cfg.upstream_url);

    // not sure why this is needed, but running in a linux/amd64
    // Docker container without this exits immediately.
    stdout().flush().unwrap();

    let handle = server.start(all_apis(jwt, cfg.jwt_expiry_secs, &cfg.upstream_url)?);
    handle.stopped().await;
    Ok(addr)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run_server().await?;
    Ok(())
}

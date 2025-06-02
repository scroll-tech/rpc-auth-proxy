mod auth;
mod proxy;

use dashmap::DashSet;
use jsonrpsee::{Methods, server::Server};
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::auth::AsyncRequireAuthorizationLayer;

use auth::{AuthenticationMiddleware, JwtSigner, SiweAuthRpcImpl, SiweAuthRpcServer};
use proxy::{EthRpcProxyImpl, EthRpcProxyServer};

const LOCAL_ENDPOINT: &str = "127.0.0.1:1234";
const REMOTE_ENDPOINT: &str = "https://rpc.scroll.io";

fn all_apis(jwt: JwtSigner) -> anyhow::Result<impl Into<Methods>> {
    let auth_server = SiweAuthRpcImpl::new(jwt);
    let proxy_server = EthRpcProxyImpl::new(REMOTE_ENDPOINT)?;
    let mut methods = auth_server.into_rpc();
    methods.merge(proxy_server.into_rpc())?;
    Ok(methods)
}

pub async fn run_server() -> anyhow::Result<SocketAddr> {
    let jwt = JwtSigner::new();

    let admin_keys = DashSet::default();
    admin_keys.insert("example_admin_token".to_owned());

    let http_middleware = ServiceBuilder::new().layer(AsyncRequireAuthorizationLayer::new(
        AuthenticationMiddleware::new(jwt.clone(), admin_keys),
    ));

    let server = Server::builder()
        .set_http_middleware(http_middleware)
        .build(LOCAL_ENDPOINT.parse::<SocketAddr>()?)
        .await?;

    let addr = server.local_addr()?;
    println!("Server is listening on {}", addr);

    let handle = server.start(all_apis(jwt)?);
    handle.stopped().await;
    Ok(addr)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run_server().await?;

    Ok(())
}

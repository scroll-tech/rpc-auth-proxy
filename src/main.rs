mod auth;
mod config;
mod proxy;
mod service;

#[macro_use]
extern crate tracing;

use auth::{AuthenticationMiddleware, JwtSigner, SiweAuthRpcImpl, SiweAuthRpcServer};
use dashmap::DashSet;
use jsonrpsee::core::middleware::RpcServiceBuilder;
use jsonrpsee::{Methods, RpcModule, server::Server};
use proxy::{EthRpcProxyServer, RpcProxyImpl, ScrollRpcProxyServer};
use std::iter::once;
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::auth::AsyncRequireAuthorizationLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use service::{RpcLoggerMiddleware, log_request};

async fn all_apis(
    jwt: JwtSigner,
    jwt_expiry_secs: usize,
    upstream_url: &str,
) -> anyhow::Result<impl Into<Methods>> {
    let auth_server = SiweAuthRpcImpl::new(jwt, jwt_expiry_secs);
    let eth_proxy_server = RpcProxyImpl::new(upstream_url)?;
    let scroll_proxy_server = RpcProxyImpl::new(upstream_url)?;

    let mut module = RpcModule::new(());
    module.merge(SiweAuthRpcServer::into_rpc(auth_server))?;
    module.merge(EthRpcProxyServer::into_rpc(eth_proxy_server))?;
    module.merge(ScrollRpcProxyServer::into_rpc(scroll_proxy_server))?;
    Ok(module)
}

pub async fn run_server() -> anyhow::Result<SocketAddr> {
    let cfg = config::load_config()?;
    let jwt = JwtSigner::from_config(cfg.jwt_signer_keys.as_slice(), &cfg.default_kid)?;

    // Only load admin_keys from config file
    let admin_keys = DashSet::default();
    for key in cfg.admin_keys {
        admin_keys.insert(key);
    }
    debug!("Loaded {} admin keys", admin_keys.len());

    let http_middleware = ServiceBuilder::new()
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetSensitiveRequestHeadersLayer::new(once(
            hyper::header::AUTHORIZATION,
        )))
        .layer(TraceLayer::new_for_http().make_span_with(log_request))
        .layer(AsyncRequireAuthorizationLayer::new(
            AuthenticationMiddleware::new(jwt.clone(), admin_keys),
        ));

    let rpc_middleware = RpcServiceBuilder::new().layer_fn(RpcLoggerMiddleware::new);

    let server = Server::builder()
        .set_http_middleware(http_middleware)
        .set_rpc_middleware(rpc_middleware)
        .build(cfg.bind_address.parse::<SocketAddr>()?)
        .await?;

    let addr = server.local_addr()?;
    info!("Server is listening on {addr}");
    info!("Upstream endpoint is {}", cfg.upstream_url);

    let methods = all_apis(jwt, cfg.jwt_expiry_secs, &cfg.upstream_url).await?;
    let handle = server.start(methods);
    handle.stopped().await;
    Ok(addr)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    match run_server().await {
        Ok(_) => Ok(()),
        Err(err) => {
            error!("Error starting server: {}", err);
            std::process::exit(1);
        }
    }
}

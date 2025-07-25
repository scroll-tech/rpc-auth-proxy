use alloy::primitives::Bytes;
use chrono::Utc;
use jsonrpsee::core::{RpcResult, async_trait};
use jsonrpsee::proc_macros::rpc;
use moka::future::Cache;
use rand::distr::{Alphanumeric, SampleString};
use siwe::{Message, VerificationOpts};
use std::sync::Arc;
use std::time::Duration;

use super::error::{internal_error, invalid_params};
use super::jwt::JwtSigner;

type NonceCache = Arc<Cache<String, ()>>;

const NONCE_SIZE: usize = 64;

#[rpc(server, client, namespace = "siwe")]
pub trait SiweAuthRpc {
    #[method(name = "getNonce")]
    async fn get_nonce(&self) -> RpcResult<String>;

    #[method(name = "signIn")]
    async fn sign_in(&self, message: String, signature: Bytes) -> RpcResult<String>;
}

pub struct SiweAuthRpcImpl {
    cache: NonceCache,
    jwt: JwtSigner,
    // JWT expiration time in seconds, timeout is not exact, there is a 60s leeway by default.
    jwt_expiry_secs: usize,
}

impl SiweAuthRpcImpl {
    pub fn new(jwt: JwtSigner, jwt_expiry_secs: usize) -> Self {
        let cache: NonceCache = Arc::new(
            Cache::builder()
                .time_to_live(Duration::from_secs(300))
                .max_capacity(10_000)
                .build(),
        );

        Self {
            cache,
            jwt,
            jwt_expiry_secs,
        }
    }
}

#[async_trait]
impl SiweAuthRpcServer for SiweAuthRpcImpl {
    async fn get_nonce(&self) -> RpcResult<String> {
        let nonce = Alphanumeric.sample_string(&mut rand::rng(), NONCE_SIZE);
        self.cache.insert(nonce.clone(), ()).await;
        Ok(nonce)
    }

    async fn sign_in(&self, message: String, signature: Bytes) -> RpcResult<String> {
        let message = match message.parse::<Message>() {
            Ok(m) => m,
            Err(e) => return Err(invalid_params(format!("invalid message: {e}"))),
        };

        if self.cache.remove(&message.nonce).await.is_none() {
            return Err(invalid_params(format!(
                "invalid message nonce: {}",
                message.nonce
            )));
        }

        // TODO: make verification strict
        let opts = VerificationOpts::default();

        if let Err(_e) = message.verify(&signature, &opts).await {
            return Err(invalid_params("invalid message or signature"));
        }

        let exp = (Utc::now().timestamp() as usize) + self.jwt_expiry_secs;
        match self.jwt.create_token(message.address, exp) {
            Ok(token) => Ok(token),
            Err(_) => Err(internal_error("unable to issue token")),
        }
    }
}

use alloy::primitives::Address;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand::distr::{Alphanumeric, SampleString};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct Keys {
    encoding: EncodingKey,
    decoding: DecodingKey,
}

impl Keys {
    fn new(secret: &[u8]) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct UserClaims {
    pub address: Address,
    pub exp: usize,
}

#[derive(Clone)]
pub struct JwtSigner {
    keys: Arc<Keys>,
}

impl JwtSigner {
    pub fn new() -> Self {
        let secret = Alphanumeric.sample_string(&mut rand::rng(), 60);
        let keys = Arc::new(Keys::new(secret.as_bytes()));
        Self { keys }
    }

    pub(super) fn create_token(&self, addr: impl Into<Address>) -> anyhow::Result<String> {
        let address: Address = addr.into();

        // note: timeout is not exact, there is a 60s leeway by default
        let timeout = Duration::from_secs(30);

        let exp = SystemTime::now()
            .checked_add(timeout)
            .expect("within bounds")
            .duration_since(UNIX_EPOCH)
            .expect("long after epoch")
            .as_secs() as usize;

        let claims = UserClaims { address, exp };
        let token = encode(&Header::default(), &claims, &self.keys.encoding)?;
        Ok(token)
    }

    pub(super) fn decode_token(&self, token: impl AsRef<str>) -> anyhow::Result<UserClaims> {
        let token_data = decode(token.as_ref(), &self.keys.decoding, &Validation::default())?;
        Ok(token_data.claims)
    }
}

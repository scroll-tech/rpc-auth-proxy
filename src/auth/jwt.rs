use std::collections::HashMap;
use std::sync::Arc;

use alloy::primitives::Address;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, decode_header, encode};
use serde::{Deserialize, Serialize};

/// Represents the JWT claims for a user
#[derive(Debug, Serialize, Deserialize)]
pub struct UserClaims {
    pub address: Address,
    pub exp: usize,
}

/// Represents a single signing key entry
struct KeyEntry {
    encoding: EncodingKey,
    decoding: DecodingKey,
    kid: String,
}

impl From<&JwtSignerKeyConfig> for KeyEntry {
    fn from(config: &JwtSignerKeyConfig) -> Self {
        KeyEntry {
            encoding: EncodingKey::from_secret(config.secret.as_bytes()),
            decoding: DecodingKey::from_secret(config.secret.as_bytes()),
            kid: config.kid.clone(),
        }
    }
}

/// Configuration for a JWT signer key
#[derive(Debug, Deserialize)]
pub struct JwtSignerKeyConfig {
    pub kid: String,
    pub secret: String,
}

/// The JwtSigner supports multiple keys and signs tokens with the configured default key
#[derive(Clone)]
pub struct JwtSigner {
    keys: Arc<HashMap<String, KeyEntry>>,
    default_kid: String, // The key used for signing new tokens
}

impl JwtSigner {
    /// Build JwtSigner from key config and a default_kid
    pub fn from_config(keys: &[JwtSignerKeyConfig], default_kid: &str) -> anyhow::Result<Self> {
        let map: HashMap<_, _> = keys.iter().map(|k| (k.kid.clone(), k.into())).collect();

        // Ensure default_kid exists in the key map
        if !map.contains_key(default_kid) {
            anyhow::bail!("default_kid '{}' not found in jwt_signer_keys", default_kid);
        }

        Ok(Self {
            keys: Arc::new(map),
            default_kid: default_kid.to_owned(),
        })
    }

    /// Create a JWT token using the default signing key
    pub fn create_token(&self, addr: impl Into<Address>, exp: usize) -> anyhow::Result<String> {
        let entry = self
            .keys
            .get(&self.default_kid)
            .ok_or_else(|| anyhow::anyhow!("Current signing key not found"))?;

        // Store key ID in the JWT header
        let mut header = Header::default();
        header.kid = Some(entry.kid.clone());

        let claims = UserClaims {
            address: addr.into(),
            exp,
        };

        let token = encode(&header, &claims, &entry.encoding)?;
        Ok(token)
    }

    /// Decode and verify a JWT token using the correct key (looked up by kid)
    pub fn decode_token(&self, token: impl AsRef<str>) -> anyhow::Result<UserClaims> {
        let kid = decode_header(token.as_ref())?
            .kid
            .ok_or_else(|| anyhow::anyhow!("No kid in JWT header"))?;

        let entry = self
            .keys
            .get(&kid)
            .ok_or_else(|| anyhow::anyhow!("JWT signing key kid {} not found", kid))?;

        let token_data = decode(token.as_ref(), &entry.decoding, &Validation::default())?;
        Ok(token_data.claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::Address;
    use std::str::FromStr;

    // Returns a sample EVM address
    fn address() -> Address {
        Address::from_str("0x1234567890abcdef1234567890abcdef12345678").unwrap()
    }

    #[test]
    fn test_jwt_signer_key_rotation() {
        // Two keys, default is key-2025-07
        let keys = vec![
            JwtSignerKeyConfig {
                kid: "key-2025-07".to_string(),
                secret: "supersecret1".to_string(),
            },
            JwtSignerKeyConfig {
                kid: "key-2025-06".to_string(),
                secret: "supersecret2".to_string(),
            },
        ];

        // Sign and decode with key-2025-07
        let signer = JwtSigner::from_config(&keys, "key-2025-07").unwrap();
        let exp = (chrono::Utc::now().timestamp() + 3600) as usize;
        let token = signer.create_token(address(), exp).unwrap();
        let claims = signer.decode_token(&token).unwrap();
        assert_eq!(claims.exp, exp);
        assert_eq!(claims.address, address());

        // Rotate: only key-2025-06 remains
        let keys_rotated = vec![JwtSignerKeyConfig {
            kid: "key-2025-06".to_string(),
            secret: "supersecret2".to_string(),
        }];
        let signer_rotated = JwtSigner::from_config(&keys_rotated, "key-2025-06").unwrap();

        // Old token can't be decoded (missing key-2025-07)
        let err = signer_rotated.decode_token(&token).unwrap_err().to_string();
        assert!(err.contains("JWT signing key kid key-2025-07 not found"));

        // New token with key-2025-06 works
        let token2 = signer_rotated.create_token(address(), exp).unwrap();
        let claims2 = signer_rotated.decode_token(&token2).unwrap();
        assert_eq!(claims2.exp, exp);
        assert_eq!(claims2.address, address());
    }
}

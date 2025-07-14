use dashmap::DashSet;
use futures_util::future::BoxFuture;
use headers::{Authorization, HeaderMapExt, authorization::Bearer};
use jsonrpsee::http_client::{HeaderMap, HttpBody, HttpRequest, HttpResponse};
use tower_http::auth::AsyncAuthorizeRequest;

use super::access_level::AccessLevel;
use super::jwt::JwtSigner;

#[derive(Clone)]
pub struct AuthenticationMiddleware {
    jwt: JwtSigner,
    api_keys: DashSet<String>,
}

impl AuthenticationMiddleware {
    pub fn new(jwt: JwtSigner, api_keys: impl Iterator<Item = String>) -> Self {
        Self {
            jwt,
            api_keys: DashSet::from_iter(api_keys),
        }
    }

    async fn authenticate_user(&self, headers: &HeaderMap) -> AccessLevel {
        let token = match headers.typed_get::<Authorization<Bearer>>() {
            Some(Authorization(bearer)) => bearer.token().to_string(),
            _ => return AccessLevel::None,
        };

        if self.api_keys.contains(&token) {
            return AccessLevel::Full;
        }

        match self.jwt.decode_token(&token) {
            Ok(claims) => AccessLevel::Basic(claims.address),
            Err(_) => AccessLevel::None,
        }
    }
}

impl AsyncAuthorizeRequest<HttpBody> for AuthenticationMiddleware {
    type RequestBody = HttpBody;
    type ResponseBody = HttpBody;
    type Future = BoxFuture<'static, Result<HttpRequest, HttpResponse>>;

    fn authorize(&mut self, mut request: HttpRequest) -> Self::Future {
        let self_clone = self.clone();
        Box::pin(async move {
            let access = self_clone.authenticate_user(request.headers()).await;
            request.extensions_mut().insert(access); // pass to rpc handler
            Ok(request)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use headers::HeaderValue;
    use jsonrpsee::http_client::HeaderMap;

    #[tokio::test]
    async fn test_admin_key_access() {
        // Simulate reading admin keys from config
        let admin_keys = vec![
            "admin-token-1-abcdefg".to_string(),
            "admin-token-2-hijklmn".to_string(),
        ];

        // Initialize admin keys
        let set = dashmap::DashSet::default();
        for key in &admin_keys {
            set.insert(key.clone());
        }

        // Create a dummy JwtSigner (not used for admin key test)
        let signer = crate::auth::JwtSigner::from_config(
            &[crate::auth::JwtSignerKeyConfig {
                kid: "test".to_string(),
                secret: "testsecret".to_string(),
            }],
            "test",
        )
        .unwrap();

        let mw = AuthenticationMiddleware::new(signer, set);

        // ----------- Test with admin key -----------
        let admin_key = &admin_keys[0];
        let mut map = HeaderMap::new();
        let value = HeaderValue::from_str(&format!("Bearer {}", admin_key)).unwrap();
        map.insert("authorization", value);

        // Should grant Full access
        let access = mw.authenticate_user(&map).await;
        assert_eq!(access, crate::auth::AccessLevel::Full);

        // ----------- Test with non-admin key -----------
        let mut map2 = HeaderMap::new();
        let value2 = HeaderValue::from_str("Bearer not-an-admin-token").unwrap();
        map2.insert("authorization", value2);

        // Should grant None access
        let access2 = mw.authenticate_user(&map2).await;
        assert_eq!(access2, crate::auth::AccessLevel::None);
    }
}

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
    pub fn new(jwt: JwtSigner, api_keys: DashSet<String>) -> Self {
        Self { jwt, api_keys }
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

use futures_util::FutureExt;
use jsonrpsee::core::middleware::{Batch, Notification, RpcServiceT};
use jsonrpsee::core::server::MethodResponse;
use jsonrpsee::types::Request;

use super::super::auth::AccessLevel;

#[derive(Clone)]
pub struct RpcLoggerMiddleware<S> {
    service: S,
}

impl<S> RpcLoggerMiddleware<S> {
    pub fn new(service: S) -> Self {
        Self { service }
    }
}

impl<S> RpcServiceT for RpcLoggerMiddleware<S>
where
    // Use the concrete MethodResponse type so that we can access the inner json.
    S: RpcServiceT<MethodResponse = MethodResponse> + Send + Sync + Clone + 'static,
{
    type MethodResponse = S::MethodResponse;
    type NotificationResponse = S::NotificationResponse;
    type BatchResponse = S::BatchResponse;

    fn call<'a>(&self, req: Request<'a>) -> impl Future<Output = Self::MethodResponse> + Send + 'a {
        let params = match &req.params {
            None => "".to_owned(),
            Some(p) => p.to_string(),
        };
        let access = req.extensions().get::<AccessLevel>();

        // log request
        info!(
            "rpc request: {}({}) (with access = {:?})",
            req.method, params, access
        );

        // execute, and lot response
        self.service.call(req).map(|resp| {
            info!("rpc response: {:}", resp.to_string());
            resp
        })
    }

    fn batch<'a>(&self, batch: Batch<'a>) -> impl Future<Output = Self::BatchResponse> + Send + 'a {
        self.service.batch(batch)
    }
    fn notification<'a>(
        &self,
        n: Notification<'a>,
    ) -> impl Future<Output = Self::NotificationResponse> + Send + 'a {
        self.service.notification(n)
    }
}

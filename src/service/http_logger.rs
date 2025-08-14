pub fn log_request<T>(request: &http::Request<T>) -> tracing::Span {
    let id = request
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-");
    tracing::info_span!("request", %id, method = ?request.method(), uri = %request.uri())
}

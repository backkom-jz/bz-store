use crate::common::response::REQUEST_ID;
use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};
use uuid::Uuid;
pub async fn wrap(req: Request, next: Next) -> Response {
    let id = Uuid::new_v4();
    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    let started = std::time::Instant::now();
    REQUEST_ID.scope(id,async move {
        let mut response=next.run(req).await;
        response.headers_mut().insert("x-request-id",HeaderValue::from_str(&id.to_string()).unwrap());
        response.headers_mut().insert("cache-control",HeaderValue::from_static("private, no-store"));
        response.headers_mut().insert("x-content-type-options",HeaderValue::from_static("nosniff"));
        response.headers_mut().insert("referrer-policy",HeaderValue::from_static("same-origin"));
        tracing::info!(request_id=%id,%method,%path,status=response.status().as_u16(),elapsed_ms=started.elapsed().as_millis(),"request completed");
        response
    }).await
}

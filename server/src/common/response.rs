use axum::Json;
use serde::Serialize;
use uuid::Uuid;
tokio::task_local! { pub static REQUEST_ID: Uuid; }
pub fn request_id() -> Uuid {
    REQUEST_ID
        .try_with(|v| *v)
        .unwrap_or_else(|_| Uuid::new_v4())
}
#[derive(Serialize)]
pub struct Envelope<T: Serialize> {
    pub code: &'static str,
    pub message: String,
    pub data: T,
    pub request_id: Uuid,
}
pub fn ok<T: Serialize>(data: T) -> Json<Envelope<T>> {
    Json(Envelope {
        code: "OK",
        message: "success".into(),
        data,
        request_id: request_id(),
    })
}

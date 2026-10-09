use super::response::{Envelope, request_id};
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
pub type ApiResult<T> = Result<T, AppError>;
#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}
impl AppError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            message,
        )
    }
    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "UNAUTHENTICATED", "请登录后继续")
    }
    pub fn forbidden() -> Self {
        Self::new(StatusCode::FORBIDDEN, "FORBIDDEN", "没有访问权限")
    }
    pub fn missing() -> Self {
        Self::new(StatusCode::NOT_FOUND, "NOT_FOUND", "资源不存在")
    }
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, "CONFLICT", message)
    }
    pub fn unavailable() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "SERVICE_UNAVAILABLE",
            "服务暂时不可用，请稍后重试",
        )
    }
    pub fn internal() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "请求处理失败",
        )
    }
}
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(Envelope {
                code: self.code,
                message: self.message,
                data: serde_json::Value::Null,
                request_id: request_id(),
            }),
        )
            .into_response()
    }
}
impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        let code = error.as_database_error().and_then(|e| e.code());
        match code.as_deref() {
            Some("23505") => Self::conflict("记录已存在或已被处理"),
            Some("23514") | Some("23503") => Self::invalid("数据不符合业务约束"),
            _ => {
                tracing::error!(kind="database",sqlstate=?code,"database operation failed");
                Self::internal()
            }
        }
    }
}
impl From<std::io::Error> for AppError {
    fn from(_: std::io::Error) -> Self {
        Self::unavailable()
    }
}
impl From<object_store::Error> for AppError {
    fn from(_: object_store::Error) -> Self {
        Self::unavailable()
    }
}
impl From<redis::RedisError> for AppError {
    fn from(_: redis::RedisError) -> Self {
        Self::unavailable()
    }
}

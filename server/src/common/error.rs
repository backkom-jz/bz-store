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
        let status = self.status;
        let mut response = (
            self.status,
            Json(Envelope {
                code: self.code,
                message: self.message,
                data: serde_json::Value::Null,
                request_id: request_id(),
            }),
        )
            .into_response();
        if status == StatusCode::TOO_MANY_REQUESTS {
            response
                .headers_mut()
                .insert("retry-after", axum::http::HeaderValue::from_static("60"));
        }
        response
    }
}
impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        let database_error = error.as_database_error();
        let code = database_error.and_then(|e| e.code());
        let mysql_number = database_error
            .and_then(|e| e.try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>())
            .map(sqlx::mysql::MySqlDatabaseError::number);
        match mysql_number {
            Some(1062) => Self::conflict("记录已存在或已被处理"),
            Some(3819 | 1451 | 1452) => Self::invalid("数据不符合业务约束"),
            _ => {
                tracing::error!(kind="database",sqlstate=?code,mysql_error_number=?mysql_number,"database operation failed");
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

use super::{dto::*, service};
use crate::{
    common::{ApiResult, AppError, extractors::ApiJson, ok},
    infrastructure::cache::limit,
    middleware::auth::{CurrentUser, cookie},
    state::AppState,
};
use axum::{
    Router,
    extract::{ConnectInfo, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use std::net::{IpAddr, SocketAddr};
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/auth/register", post(register))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/refresh", post(refresh))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/csrf", get(csrf))
        .route("/api/v1/users/me", get(me))
}
fn ip(state: &AppState, peer: ConnectInfo<SocketAddr>, headers: &HeaderMap) -> String {
    let remote = peer.0.ip();
    if state.config.trusted_proxies.contains(&remote) {
        if let Some(ip) = headers
            .get("x-real-ip")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<IpAddr>().ok())
        {
            return ip.to_string();
        }
    }
    remote.to_string()
}
fn session_response(state: &AppState, result: SessionResult, status: StatusCode) -> Response {
    let mut response = (status, ok(result.user)).into_response();
    let secure = if state.config.secure() {
        "; Secure"
    } else {
        ""
    };
    for value in [
        format!(
            "ds_access={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=900{secure}",
            result.access
        ),
        format!(
            "ds_refresh={}; Path=/api/v1/auth; HttpOnly; SameSite=Lax; Max-Age=604800{secure}",
            result.refresh
        ),
    ] {
        response
            .headers_mut()
            .append("set-cookie", HeaderValue::from_str(&value).unwrap());
    }
    response
}
async fn register(
    State(state): State<AppState>,
    peer: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    ApiJson(input): ApiJson<Credentials>,
) -> ApiResult<Response> {
    limit(&state, "register", &ip(&state, peer, &headers), 5, 3600).await?;
    Ok(session_response(
        &state,
        service::register(&state, input).await?,
        StatusCode::CREATED,
    ))
}
async fn login(
    State(state): State<AppState>,
    peer: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    ApiJson(input): ApiJson<Credentials>,
) -> ApiResult<Response> {
    limit(&state, "login-ip", &ip(&state, peer, &headers), 10, 60).await?;
    limit(
        &state,
        "login-email",
        &input.email.trim().to_lowercase(),
        10,
        900,
    )
    .await?;
    Ok(session_response(
        &state,
        service::login(&state, input).await?,
        StatusCode::OK,
    ))
}
async fn refresh(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<Response> {
    let raw = cookie(&headers, "ds_refresh").ok_or_else(AppError::unauthorized)?;
    let sid:uuid::Uuid=sqlx::query_scalar("SELECT id FROM user_sessions WHERE token_hash=$1 AND revoked_at IS NULL AND expires_at>now()").bind(service::digest(&raw)).fetch_optional(&state.pool).await?.ok_or_else(AppError::unauthorized)?;
    limit(&state, "refresh", &sid.to_string(), 30, 60).await?;
    let csrf = headers
        .get("x-csrf-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    Ok(session_response(
        &state,
        service::refresh(&state, &raw, csrf).await?,
        StatusCode::OK,
    ))
}
async fn csrf(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<impl IntoResponse> {
    let sid = if let Ok(user) = crate::middleware::auth::authenticate(&state, &headers).await {
        user.session_id
    } else {
        let raw = cookie(&headers, "ds_refresh").ok_or_else(AppError::unauthorized)?;
        sqlx::query_scalar("SELECT s.id FROM user_sessions s JOIN users u ON u.id=s.user_id WHERE s.token_hash=$1 AND s.revoked_at IS NULL AND s.expires_at>now() AND u.status=1").bind(service::digest(&raw)).fetch_optional(&state.pool).await?.ok_or_else(AppError::unauthorized)?
    };
    Ok(ok(
        serde_json::json!({"csrf_token":service::csrf_token(&state,sid)}),
    ))
}
async fn me(State(state): State<AppState>, user: CurrentUser) -> ApiResult<impl IntoResponse> {
    Ok(ok(service::public_user(&state, user.id).await?))
}
async fn logout(State(state): State<AppState>, user: CurrentUser) -> ApiResult<Response> {
    sqlx::query("UPDATE user_sessions SET revoked_at=now() WHERE id=$1")
        .bind(user.session_id)
        .execute(&state.pool)
        .await?;
    let mut response = ok(serde_json::json!({"logged_out":true})).into_response();
    for (name, path) in [("ds_access", "/"), ("ds_refresh", "/api/v1/auth")] {
        response.headers_mut().append(
            "set-cookie",
            HeaderValue::from_str(&format!(
                "{name}=; Path={path}; HttpOnly; SameSite=Lax; Max-Age=0{}",
                if state.config.secure() {
                    "; Secure"
                } else {
                    ""
                }
            ))
            .unwrap(),
        );
    }
    Ok(response)
}

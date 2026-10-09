use crate::{
    common::{ApiResult, AppError},
    modules::auth::{
        dto::Claims,
        service::{csrf_token, digest},
    },
    state::AppState,
};
use axum::{
    extract::FromRequestParts,
    http::{HeaderMap, Method, request::Parts},
};
use uuid::Uuid;
#[derive(Clone)]
pub struct CurrentUser {
    pub id: Uuid,
    pub session_id: Uuid,
    pub is_admin: bool,
}
pub fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all("cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|v| {
            let (k, value) = v.trim().split_once('=')?;
            (k == name).then(|| value.to_owned())
        })
}
pub async fn authenticate(state: &AppState, headers: &HeaderMap) -> ApiResult<CurrentUser> {
    let token = cookie(headers, "ds_access").ok_or_else(AppError::unauthorized)?;
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.set_issuer(&[&state.config.site_url]);
    validation.set_audience(&["devstore"]);
    validation.leeway = 0;
    let claims = jsonwebtoken::decode::<Claims>(
        &token,
        &jsonwebtoken::DecodingKey::from_secret(state.config.jwt_secret.as_bytes()),
        &validation,
    )
    .map_err(|_| AppError::unauthorized())?
    .claims;
    let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM user_sessions s JOIN users u ON u.id=s.user_id WHERE s.id=$1 AND s.user_id=$2 AND s.revoked_at IS NULL AND s.expires_at>now() AND u.status=1)").bind(claims.sid).bind(claims.sub).fetch_one(&state.pool).await?;
    if !valid {
        return Err(AppError::unauthorized());
    }
    let user = crate::modules::auth::service::public_user(state, claims.sub).await?;
    Ok(CurrentUser {
        id: claims.sub,
        session_id: claims.sid,
        is_admin: user.is_admin,
    })
}
pub fn verify_csrf(state: &AppState, user: &CurrentUser, headers: &HeaderMap) -> ApiResult<()> {
    use subtle::ConstantTimeEq;
    let supplied = headers
        .get("x-csrf-token")
        .and_then(|s| s.to_str().ok())
        .unwrap_or("");
    if digest(supplied)
        .as_bytes()
        .ct_eq(digest(&csrf_token(state, user.session_id)).as_bytes())
        .unwrap_u8()
        != 1
    {
        return Err(AppError::forbidden());
    }
    Ok(())
}
impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> ApiResult<Self> {
        let user = authenticate(state, &parts.headers).await?;
        if !matches!(parts.method, Method::GET | Method::HEAD | Method::OPTIONS) {
            verify_csrf(state, &user, &parts.headers)?
        }
        Ok(user)
    }
}
pub struct AdminUser(pub CurrentUser);
impl FromRequestParts<AppState> for AdminUser {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> ApiResult<Self> {
        let user = CurrentUser::from_request_parts(parts, state).await?;
        if !user.is_admin {
            return Err(AppError::forbidden());
        }
        Ok(Self(user))
    }
}

use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Credentials {
    pub email: String,
    pub password: String,
}
#[derive(Serialize, sqlx::FromRow, Clone, Debug, utoipa::ToSchema)]
pub struct PublicUser {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub is_admin: bool,
}
#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub sid: Uuid,
    pub exp: i64,
    pub iat: i64,
    pub iss: String,
    pub aud: String,
}
pub struct SessionResult {
    pub user: PublicUser,
    pub access: String,
    pub refresh: String,
}

use serde::{Deserialize, Serialize};
use uuid::Uuid;
fn draft() -> String {
    "draft".into()
}
#[derive(Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TutorialInput {
    pub product_id: Uuid,
    pub title: String,
    pub slug: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "draft")]
    pub status: String,
}
#[derive(Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ChapterInput {
    pub title: String,
    #[serde(default)]
    pub content_md: String,
    #[serde(default)]
    pub is_preview: bool,
    #[serde(default)]
    pub sort_order: i32,
    #[serde(default = "draft")]
    pub status: String,
}

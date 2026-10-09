use serde::{Deserialize, Serialize};
use uuid::Uuid;
fn tutorial() -> String {
    "tutorial".into()
}
fn draft() -> String {
    "draft".into()
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProductInput {
    pub title: String,
    pub slug: String,
    #[serde(rename = "type", default = "tutorial")]
    pub kind: String,
    pub price_cents: i64,
    #[serde(default)]
    pub description: String,
    #[serde(default = "draft")]
    pub status: String,
    #[serde(default)]
    pub category_id: Option<Uuid>,
}

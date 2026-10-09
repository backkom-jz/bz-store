use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateOrderInput {
    pub product_id: Uuid,
    pub quantity: i32,
}
#[derive(Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct PaymentConfig {
    pub channel: String,
    pub payee: String,
    pub account: String,
    pub instructions: String,
}
impl PaymentConfig {
    pub fn validate(&self) -> crate::common::ApiResult<()> {
        use crate::common::{AppError, pagination::text};
        if !matches!(self.channel.as_str(), "wechat" | "alipay" | "bank") {
            return Err(AppError::invalid("不支持的收款渠道"));
        }
        text(&self.payee, 100)?;
        text(&self.account, 200)?;
        text(&self.instructions, 2000)?;
        Ok(())
    }
}

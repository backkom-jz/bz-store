use serde::Deserialize;
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ApproveInput {
    pub actual_amount_cents: i64,
    pub payment_channel: String,
    pub transaction_reference: String,
    pub receipt_confirmed: bool,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RejectInput {
    pub reason: String,
}

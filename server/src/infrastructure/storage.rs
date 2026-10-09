use crate::{common::ApiResult, config::Config};
use object_store::aws::{AmazonS3, AmazonS3Builder};
use object_store::signer::Signer;
pub fn build(config: &Config) -> ApiResult<AmazonS3> {
    Ok(AmazonS3Builder::new()
        .with_bucket_name(&config.s3_bucket)
        .with_region("us-east-1")
        .with_endpoint(&config.s3_endpoint)
        .with_access_key_id(&config.s3_access_key)
        .with_secret_access_key(&config.s3_secret_key)
        .with_allow_http(!config.secure())
        .build()?)
}
pub async fn create_bucket(config: &Config) -> ApiResult<()> {
    let s3 = build(config)?;
    let url = s3
        .signed_url(
            reqwest::Method::PUT,
            &object_store::path::Path::from(""),
            std::time::Duration::from_secs(60),
        )
        .await?;
    let response = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|_| crate::common::AppError::internal())?
        .put(url)
        .send()
        .await
        .map_err(|_| crate::common::AppError::unavailable())?;
    if !response.status().is_success() && response.status() != reqwest::StatusCode::CONFLICT {
        return Err(crate::common::AppError::unavailable());
    }
    Ok(())
}

use crate::{common::ApiResult, config::Config};
use object_store::aws::{AmazonS3, AmazonS3Builder};
use object_store::signer::Signer;
pub fn endpoint(config: &Config) -> ApiResult<String> {
    if !config.s3_virtual_hosted {
        return Ok(config.s3_endpoint.clone());
    }
    let mut url = reqwest::Url::parse(&config.s3_endpoint)
        .map_err(|_| crate::common::AppError::invalid("S3_ENDPOINT 无效"))?;
    let host = url
        .host_str()
        .ok_or_else(|| crate::common::AppError::invalid("S3_ENDPOINT 无效"))?
        .to_owned();
    if host.split('.').next() != Some(config.s3_bucket.as_str()) {
        let bucket_host = format!("{}.{}", config.s3_bucket, host);
        url.set_host(Some(&bucket_host))
            .map_err(|_| crate::common::AppError::invalid("S3_ENDPOINT 无效"))?;
    }
    url.set_path("");
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string().trim_end_matches('/').to_string())
}
pub fn build(config: &Config) -> ApiResult<AmazonS3> {
    Ok(AmazonS3Builder::new()
        .with_bucket_name(&config.s3_bucket)
        .with_region(&config.s3_region)
        .with_endpoint(endpoint(config)?)
        .with_access_key_id(&config.s3_access_key)
        .with_secret_access_key(&config.s3_secret_key)
        .with_allow_http(!config.secure())
        .with_virtual_hosted_style_request(config.s3_virtual_hosted)
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
        .await
        .map_err(|error| {
            let detail = error.to_string();
            let detail = detail.split('?').next().unwrap_or(&detail);
            crate::common::AppError::new(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "SERVICE_UNAVAILABLE",
                format!(
                    "对象存储签名失败: {}",
                    detail.chars().take(160).collect::<String>()
                ),
            )
        })?;
    let response = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|_| crate::common::AppError::internal())?
        .put(url)
        .send()
        .await
        .map_err(|error| {
            crate::common::AppError::new(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "SERVICE_UNAVAILABLE",
                format!("无法连接对象存储: {}", error.without_url()),
            )
        })?;
    if !response.status().is_success() && response.status() != reqwest::StatusCode::CONFLICT {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let code = body
            .split("<Code>")
            .nth(1)
            .and_then(|value| value.split("</Code>").next())
            .unwrap_or("unknown");
        let message = body
            .split("<Message>")
            .nth(1)
            .and_then(|value| value.split("</Message>").next())
            .unwrap_or("");
        return Err(crate::common::AppError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "SERVICE_UNAVAILABLE",
            format!("对象存储初始化失败（HTTP {status} {code}）{message}"),
        ));
    }
    Ok(())
}

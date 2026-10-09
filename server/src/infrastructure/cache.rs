use crate::{
    common::{ApiResult, AppError},
    modules::auth::service::digest,
    state::AppState,
};
pub async fn limit(
    state: &AppState,
    bucket: &str,
    identity: &str,
    max: i64,
    seconds: i64,
) -> ApiResult<()> {
    let key = format!(
        "devstore:{}:{}:{}",
        state.namespace,
        bucket,
        digest(identity)
    );
    let count:i64=redis::Script::new("local n=redis.call('INCR',KEYS[1]); if n==1 then redis.call('EXPIRE',KEYS[1],ARGV[1]); end; return n")
 .key(key).arg(seconds).invoke_async(&mut state.redis.clone()).await?;
    if count > max {
        return Err(AppError::new(
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            "RATE_LIMITED",
            "请求过于频繁，请稍后重试",
        ));
    }
    Ok(())
}

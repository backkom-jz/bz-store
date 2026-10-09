use super::{ApiResult, AppError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ListQuery {
    pub q: Option<String>,
    pub category: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub status: Option<String>,
}
pub struct Pagination {
    pub q: String,
    pub category: String,
    pub page: i64,
    pub size: i64,
    pub offset: i64,
    pub status: String,
}
impl ListQuery {
    pub fn validate(self) -> ApiResult<Pagination> {
        let q = self.q.unwrap_or_default().trim().to_owned();
        let page = self.page.unwrap_or(1);
        let size = self.page_size.unwrap_or(20);
        if q.chars().count() > 100 || !(1..=1_000_000).contains(&page) || !(1..=100).contains(&size)
        {
            return Err(AppError::invalid("分页或搜索参数超出范围"));
        }
        Ok(Pagination {
            q,
            category: self.category.unwrap_or_default(),
            page,
            size,
            offset: (page - 1) * size,
            status: self.status.unwrap_or_default(),
        })
    }
}
pub fn patch<T: serde::de::DeserializeOwned + Serialize>(old: T, changes: Value) -> ApiResult<T> {
    let mut base = serde_json::to_value(old).map_err(|_| AppError::internal())?;
    let fields = changes
        .as_object()
        .ok_or_else(|| AppError::invalid("修改内容必须是对象"))?;
    for (key, value) in fields {
        if !base.as_object().unwrap().contains_key(key) {
            return Err(AppError::invalid("包含不支持的字段"));
        }
        base[key] = value.clone();
    }
    serde_json::from_value(base).map_err(|_| AppError::invalid("字段类型无效"))
}
pub fn text(value: &str, max: usize) -> ApiResult<()> {
    if value.trim().is_empty() || value.chars().count() > max {
        return Err(AppError::invalid("必填内容为空或超过长度限制"));
    }
    Ok(())
}
pub fn slug(value: &str) -> ApiResult<()> {
    if value.len() > 220
        || value.is_empty()
        || value.starts_with('-')
        || value.ends_with('-')
        || value.contains("--")
        || !value
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
    {
        return Err(AppError::invalid("slug 仅可使用小写字母、数字和单连字符"));
    }
    Ok(())
}

use crate::{
    common::{ApiResult, AppError},
    middleware::auth::CurrentUser,
    modules::{admin::audit, orders::dto::PaymentConfig},
    state::AppState,
};
use serde_json::{Value, json};
use uuid::Uuid;
pub async fn seed_demo(state: &AppState, actor: &CurrentUser) -> ApiResult<Value> {
    if !actor.is_admin {
        return Err(AppError::forbidden());
    }
    let mut tx = state.pool.begin().await?;
    // Serialize explicit operator seed calls without modifying user-edited demo content.
    sqlx::query("SELECT id FROM roles WHERE code='admin' FOR UPDATE")
        .execute(&mut *tx)
        .await?;
    if let Some(value) = sqlx::query_scalar::<_, Value>(
        "SELECT JSON_OBJECT('id',BIN_TO_UUID(id),'category_id',BIN_TO_UUID(category_id),'type',type,'title',title,'slug',slug,'description',description,'price_cents',price_cents,'currency',currency,'status',status,'is_demo',is_demo,'created_at',created_at,'updated_at',updated_at) FROM products WHERE slug='rust-axum-starter'",
    )
    .fetch_optional(&mut *tx)
    .await?
    {
        return Ok(value);
    }
    let category = Uuid::new_v4();
    sqlx::query("INSERT INTO categories(id,name,slug) VALUES(?,'Rust 开发','rust') ON DUPLICATE KEY UPDATE slug=VALUES(slug)").bind(category).execute(&mut *tx).await?;
    let category: Uuid = sqlx::query_scalar("SELECT id FROM categories WHERE slug='rust'")
        .fetch_one(&mut *tx)
        .await?;
    let product = Uuid::new_v4();
    sqlx::query("INSERT INTO products(id,title,slug,type,price_cents,description,is_demo,category_id) VALUES(?,'Rust + Axum：从接口到交易闭环','rust-axum-starter','tutorial',9900,'示例教程：围绕实际项目，学习类型安全的 API、数据库事务和部署。此内容用于本地演示。',true,?)").bind(product).bind(category).execute(&mut *tx).await?;
    let tutorial = Uuid::new_v4();
    sqlx::query("INSERT INTO tutorials(id,product_id,title,slug,description,status) VALUES(?,?,'Rust + Axum：从接口到交易闭环','rust-axum-starter','示例教程，包含免费预览和购买后的项目实践。','published')").bind(tutorial).bind(product).execute(&mut *tx).await?;
    for (sort, title, content, preview) in [
        (
            0,
            "01 · 从一个清晰的接口开始",
            "# 从一个清晰的接口开始\n\n一个值得维护的 API，应当让调用者理解成功与失败。先约定输入、输出和错误，再编写路由。\n\n```rust\nasync fn health() -> &'static str {\n    \"ready\"\n}\n```\n\n这是免费预览章节。后续章节用事务将订单与权益连接起来。",
            true,
        ),
        (
            1,
            "02 · 用事务完成一次授权",
            "# 用事务完成一次授权\n\n付款审核涉及订单、凭证、权益和审计。这些写入要么一起成功，要么一起回滚。\n\n```rust\nlet mut tx = pool.begin().await?;\n// 锁定订单，核对状态与实际到账金额。\n// 更新凭证，授予权益，写入审计。\ntx.commit().await?;\n```\n\n对同一订单重复审核，应该返回当前结果。唯一约束是最后一道保护，不能只依赖前端禁用按钮。\n\n## 下一步\n\n尝试并发发起两次审核，并确认只产生一个权益。",
            false,
        ),
    ] {
        sqlx::query("INSERT INTO tutorial_chapters(id,tutorial_id,title,content_md,is_preview,sort_order,status) VALUES(?,?,?,?,?,?,'published')").bind(Uuid::new_v4()).bind(tutorial).bind(title).bind(content).bind(preview).bind(sort).execute(&mut *tx).await?;
    }
    sqlx::query("UPDATE products SET status='published' WHERE id=?")
        .bind(product)
        .execute(&mut *tx)
        .await?;
    audit::record(
        &mut tx,
        actor.id,
        "demo.seed",
        product,
        json!({"source":"local_cli"}),
    )
    .await?;
    let value = sqlx::query_scalar("SELECT JSON_OBJECT('id',BIN_TO_UUID(p.id),'category_id',BIN_TO_UUID(p.category_id),'type',p.type,'title',p.title,'slug',p.slug,'description',p.description,'price_cents',p.price_cents,'currency',p.currency,'status',p.status,'is_demo',p.is_demo,'created_at',p.created_at,'updated_at',p.updated_at) FROM products p WHERE id=?")
        .bind(product)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(value)
}
pub async fn payment_config(
    state: &AppState,
    actor: &CurrentUser,
    value: PaymentConfig,
) -> ApiResult<()> {
    if !actor.is_admin {
        return Err(AppError::forbidden());
    }
    value.validate()?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("INSERT INTO site_settings(setting_key,value_json) VALUES('manual_payment',?) ON DUPLICATE KEY UPDATE value_json=VALUES(value_json)").bind(serde_json::to_value(&value).map_err(|_|AppError::internal())?).execute(&mut *tx).await?;
    audit::record(
        &mut tx,
        actor.id,
        "payment_config.update",
        actor.id,
        json!({"channel":value.channel,"source":"local_cli"}),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

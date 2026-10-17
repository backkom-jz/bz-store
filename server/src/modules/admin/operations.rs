use crate::{
    common::{ApiResult, AppError},
    middleware::auth::CurrentUser,
    modules::{admin::audit, orders::dto::PaymentConfig},
    state::AppState,
};
use object_store::{ObjectStore, path::Path};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const DEMO_PDF: &[u8] = b"%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Count 1/Kids[3 0 R]>>endobj\n3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 300 144]/Contents 4 0 R>>endobj\n4 0 obj<</Length 55>>stream\nBT /F1 18 Tf 36 80 Td (DevStore demo file) Tj ET\nendstream\nendobj\ntrailer<</Root 1 0 R>>\n%%EOF\n";

pub async fn seed_demo(state: &AppState, actor: &CurrentUser) -> ApiResult<Value> {
    if !actor.is_admin {
        return Err(AppError::forbidden());
    }
    let mut tx = state.pool.begin().await?;
    // Serialize explicit operator seed calls without modifying user-edited demo content.
    sqlx::query("SELECT id FROM roles WHERE code='admin' FOR UPDATE")
        .execute(&mut *tx)
        .await?;
    let category = Uuid::new_v4();
    sqlx::query("INSERT INTO categories(id,name,slug) VALUES(?,'Rust 开发','rust') ON DUPLICATE KEY UPDATE slug=VALUES(slug)").bind(category).execute(&mut *tx).await?;
    let category: Uuid = sqlx::query_scalar("SELECT id FROM categories WHERE slug='rust'")
        .fetch_one(&mut *tx)
        .await?;
    let tutorial_existed: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM products WHERE slug='rust-axum-starter')")
            .fetch_one(&mut *tx)
            .await?;
    let product = if tutorial_existed {
        sqlx::query_scalar("SELECT id FROM products WHERE slug='rust-axum-starter'")
            .fetch_one(&mut *tx)
            .await?
    } else {
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
        product
    };
    let notes = ensure_product(
        &mut tx,
        category,
        "demo-rust-notes",
        "digital",
        0,
        "Rust 速查笔记",
        "示例数字商品：零元领取后可以下载一份 PDF。此内容用于本地演示。",
    )
    .await?;
    let review = ensure_product(
        &mut tx,
        category,
        "demo-architecture-review",
        "service",
        0,
        "架构评审服务",
        "示例服务：领取后会打开一张工单。此内容用于本地演示。",
    )
    .await?;
    let retired = ensure_product(
        &mut tx,
        category,
        "demo-retired-sample",
        "digital",
        0,
        "已退款的示例文件",
        "示例：对应订单已退款，再次下载会被拒绝。此内容用于本地演示。",
    )
    .await?;
    let mut uploads = Vec::new();
    for product_id in [notes, retired] {
        if let Some(key) = ensure_demo_file(&mut tx, actor.id, product_id).await? {
            uploads.push(key);
        }
    }
    ensure_blog(&mut tx).await?;
    ensure_sample_order(
        &mut tx,
        actor.id,
        notes,
        "demo-rust-notes",
        "digital",
        "Rust 速查笔记",
        "completed",
    )
    .await?;
    ensure_sample_order(
        &mut tx,
        actor.id,
        review,
        "demo-architecture-review",
        "service",
        "架构评审服务",
        "fulfilling",
    )
    .await?;
    ensure_sample_order(
        &mut tx,
        actor.id,
        retired,
        "demo-retired-sample",
        "digital",
        "已退款的示例文件",
        "refunded",
    )
    .await?;
    if !tutorial_existed {
        audit::record(
            &mut tx,
            actor.id,
            "demo.seed",
            product,
            json!({"source":"local_cli"}),
        )
        .await?;
    }
    let value = sqlx::query_scalar("SELECT JSON_OBJECT('id',BIN_TO_UUID(p.id),'category_id',BIN_TO_UUID(p.category_id),'type',p.type,'title',p.title,'slug',p.slug,'description',p.description,'price_cents',p.price_cents,'currency',p.currency,'status',p.status,'is_demo',p.is_demo,'created_at',p.created_at,'updated_at',p.updated_at) FROM products p WHERE id=?")
        .bind(product)
        .fetch_one(&mut *tx)
        .await?;
    for key in &uploads {
        state
            .storage
            .put(&Path::from(key.as_str()), DEMO_PDF.to_vec().into())
            .await
            .map_err(|_| AppError::unavailable())?;
    }
    tx.commit().await?;
    Ok(value)
}

async fn ensure_product(
    tx: &mut sqlx::MySqlConnection,
    category: Uuid,
    slug: &str,
    kind: &str,
    price: i64,
    title: &str,
    description: &str,
) -> ApiResult<Uuid> {
    if let Some(id) = sqlx::query_scalar("SELECT id FROM products WHERE slug=?")
        .bind(slug)
        .fetch_optional(&mut *tx)
        .await?
    {
        return Ok(id);
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO products(id,title,slug,type,price_cents,description,status,is_demo,category_id) VALUES(?,?,?,?,?,?, 'published', true, ?)")
        .bind(id).bind(title).bind(slug).bind(kind).bind(price).bind(description).bind(category)
        .execute(&mut *tx).await?;
    Ok(id)
}

async fn ensure_demo_file(
    tx: &mut sqlx::MySqlConnection,
    owner: Uuid,
    product_id: Uuid,
) -> ApiResult<Option<String>> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM product_files WHERE product_id=?)")
            .bind(product_id)
            .fetch_one(&mut *tx)
            .await?;
    if exists {
        return Ok(None);
    }
    let file_id = Uuid::new_v4();
    let key = format!("digital/{file_id}");
    sqlx::query("INSERT INTO file_objects(id,owner_id,purpose,storage_key,content_type,size_bytes,sha256,status) VALUES(?,?,'digital_asset',?,?,?,?,'clean')")
        .bind(file_id).bind(owner).bind(&key).bind("application/pdf").bind(DEMO_PDF.len() as i64).bind(format!("{:x}", Sha256::digest(DEMO_PDF)))
        .execute(&mut *tx).await?;
    sqlx::query("INSERT INTO product_files(id,product_id,file_id) VALUES(?,?,?)")
        .bind(Uuid::new_v4())
        .bind(product_id)
        .bind(file_id)
        .execute(&mut *tx)
        .await?;
    Ok(Some(key))
}

async fn ensure_blog(tx: &mut sqlx::MySqlConnection) -> ApiResult<()> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM blog_posts WHERE slug='shipping-digital-goods')",
    )
    .fetch_one(&mut *tx)
    .await?;
    if exists {
        return Ok(());
    }
    sqlx::query("INSERT INTO blog_posts(id,title,slug,excerpt,content_md,status,published_at) VALUES(?,?,?,?,?,'published',UTC_TIMESTAMP(6))")
        .bind(Uuid::new_v4())
        .bind("数字商品怎么交付")
        .bind("shipping-digital-goods")
        .bind("示例文章：文件不公开，每次下载都核对权益。")
        .bind("# 数字商品怎么交付\n\n买家付款后得到的是账号里的权益，不是一个长期有效的公开链接。\n\n每次下载都由服务端再核对一次。退款登记完成后，下一次下载会失败。\n\n这篇文章只用于本地演示，不进入商品目录。")
        .execute(&mut *tx).await?;
    Ok(())
}

async fn ensure_sample_order(
    tx: &mut sqlx::MySqlConnection,
    user_id: Uuid,
    product_id: Uuid,
    slug: &str,
    kind: &str,
    title: &str,
    status: &str,
) -> ApiResult<()> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM order_items i JOIN orders o ON o.id=i.order_id WHERE i.product_id=? AND o.user_id=?)").bind(product_id).bind(user_id).fetch_one(&mut *tx).await?;
    if exists {
        return Ok(());
    }
    let order_id = Uuid::new_v4();
    let item_id = Uuid::new_v4();
    let order_no = format!("DS{}", order_id.simple().to_string().to_uppercase());
    sqlx::query("INSERT INTO orders(id,order_no,user_id,amount_cents,status,expires_at,paid_at) VALUES(?,?,?,0,?,DATE_ADD(UTC_TIMESTAMP(6), INTERVAL 24 HOUR),UTC_TIMESTAMP(6))")
        .bind(order_id).bind(order_no).bind(user_id).bind(status).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO order_items(id,order_id,product_id,product_title,product_type,unit_price_cents,quantity,subtotal_cents) VALUES(?,?,?,?,?,0,1,0)")
        .bind(item_id).bind(order_id).bind(product_id).bind(title).bind(kind).execute(&mut *tx).await?;
    if status == "completed" {
        sqlx::query("INSERT INTO user_entitlements(id,user_id,order_item_id,product_id,status) VALUES(?,?,?,?,'active')")
            .bind(Uuid::new_v4()).bind(user_id).bind(item_id).bind(product_id).execute(&mut *tx).await?;
    }
    if status == "fulfilling" {
        crate::modules::tickets::open_ticket(&mut *tx, order_id, item_id, user_id).await?;
        sqlx::query("INSERT INTO ticket_messages(id,ticket_id,author_id,body) SELECT ?,id,?,? FROM service_tickets WHERE order_id=?")
            .bind(Uuid::new_v4()).bind(user_id).bind("示例回复：交付说明可以写在这条工单里。").bind(order_id).execute(&mut *tx).await?;
    }
    if status == "refunded" {
        sqlx::query("INSERT INTO user_entitlements(id,user_id,order_item_id,product_id,status,revoked_at) VALUES(?,?,?,?,'revoked',UTC_TIMESTAMP(6))")
            .bind(Uuid::new_v4()).bind(user_id).bind(item_id).bind(product_id).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO refunds(id,order_id,amount_cents,reference_no,note,actor_id) VALUES(?,?,0,?,?,?)")
            .bind(Uuid::new_v4()).bind(order_id).bind(format!("DEMO-{}-{slug}", order_id.simple())).bind("示例退款，用于核对权益已经收回。").bind(user_id)
            .execute(&mut *tx).await?;
    }
    Ok(())
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

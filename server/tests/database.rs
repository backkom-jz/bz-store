mod support;
use sqlx::Row;
use uuid::Uuid;

#[tokio::test]
async fn mysql_database_constraints_and_idempotent_entitlements() {
    let pool = support::isolated_pool().await;
    let table: Option<String> = sqlx::query_scalar(
        "SELECT TABLE_NAME FROM information_schema.TABLES WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='orders'",
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert_eq!(table.as_deref(), Some("orders"));

    let user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,email,password_hash) VALUES(?,?,?)")
        .bind(user_id)
        .bind("a@example.test")
        .bind("hash")
        .execute(&pool)
        .await
        .unwrap();
    let product_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO products(id,title,slug,type,price_cents) VALUES(?,?,'test','tutorial',9900)",
    )
    .bind(product_id)
    .bind("Test")
    .execute(&pool)
    .await
    .unwrap();
    let order_id = Uuid::new_v4();
    sqlx::query("INSERT INTO orders(id,order_no,user_id,amount_cents,expires_at) VALUES(?,?,?,9900,DATE_ADD(UTC_TIMESTAMP(6),INTERVAL 1 DAY))")
        .bind(order_id)
        .bind(format!("DS{}", order_id.simple()))
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    let bad_item = sqlx::query("INSERT INTO order_items(id,order_id,product_id,product_title,product_type,unit_price_cents,quantity,subtotal_cents) VALUES(?,?,?,'Test','tutorial',9900,2,19800)")
        .bind(Uuid::new_v4())
        .bind(order_id)
        .bind(product_id)
        .execute(&pool)
        .await;
    assert!(bad_item.is_err(), "quantity constraint must be enforced");

    let item_id = Uuid::new_v4();
    sqlx::query("INSERT INTO order_items(id,order_id,product_id,product_title,product_type,unit_price_cents,quantity,subtotal_cents) VALUES(?,?,?,'Test','tutorial',9900,1,9900)")
        .bind(item_id)
        .bind(order_id)
        .bind(product_id)
        .execute(&pool)
        .await
        .unwrap();
    for _ in 0..2 {
        sqlx::query("INSERT IGNORE INTO user_entitlements(id,user_id,order_item_id,product_id) VALUES(?,?,?,?)")
            .bind(Uuid::new_v4())
            .bind(user_id)
            .bind(item_id)
            .bind(product_id)
            .execute(&pool)
            .await
            .unwrap();
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_entitlements")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let _: Uuid = sqlx::query("SELECT id FROM users WHERE id=?")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("id");
}

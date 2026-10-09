mod support;
use sqlx::Row;

#[tokio::test]
async fn it_database_constraints() {
    let pool = support::isolated_pool().await;
    let table: Option<String> = sqlx::query_scalar("SELECT to_regclass('orders')::text")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(table.is_some(), "orders migration must exist");
    let user: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO users(email,password_hash) VALUES('a@example.test','hash') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let product: uuid::Uuid = sqlx::query_scalar("INSERT INTO products(title,slug,type,price_cents) VALUES('Test','test','tutorial',9900) RETURNING id").fetch_one(&pool).await.unwrap();
    for (price, currency) in [(-1, "CNY"), (9900, "USD"), (100_000_001, "CNY")] {
        let err = sqlx::query("INSERT INTO orders(order_no,user_id,amount_cents,currency) VALUES(gen_random_uuid()::text,$1,$2,$3)")
            .bind(user).bind(price as i64).bind(currency).execute(&pool).await.unwrap_err();
        assert_eq!(
            err.as_database_error().unwrap().code().as_deref(),
            Some("23514")
        );
    }
    let order: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO orders(order_no,user_id,amount_cents) VALUES('DS-TEST',$1,9900) RETURNING id",
    )
    .bind(user)
    .fetch_one(&pool)
    .await
    .unwrap();
    let err = sqlx::query("INSERT INTO order_items(order_id,product_id,product_title,product_type,unit_price_cents,quantity,subtotal_cents) VALUES($1,$2,'Test','tutorial',9900,2,19800)").bind(order).bind(product).execute(&pool).await.unwrap_err();
    assert_eq!(
        err.as_database_error().unwrap().code().as_deref(),
        Some("23514")
    );
    let row=sqlx::query("INSERT INTO order_items(order_id,product_id,product_title,product_type,unit_price_cents,quantity,subtotal_cents) VALUES($1,$2,'Test','tutorial',9900,1,9900) RETURNING id").bind(order).bind(product).fetch_one(&pool).await.unwrap();
    let item: uuid::Uuid = row.get("id");
    for _ in 0..2 {
        sqlx::query("INSERT INTO user_entitlements(user_id,order_item_id,product_id) VALUES($1,$2,$3) ON CONFLICT(order_item_id) DO NOTHING").bind(user).bind(item).bind(product).execute(&pool).await.unwrap();
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM user_entitlements")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let duplicate =
        sqlx::query("INSERT INTO users(email,password_hash) VALUES('A@EXAMPLE.TEST','hash')")
            .execute(&pool)
            .await
            .unwrap_err();
    assert_eq!(
        duplicate.as_database_error().unwrap().code().as_deref(),
        Some("23514")
    );
}

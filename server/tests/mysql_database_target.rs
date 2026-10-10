#[path = "support/database.rs"]
mod database;

#[test]
fn test_database_url_rejects_runtime_and_erp_database_names() {
    for raw in [
        "mysql://user:secret@db.example.test:3306/bz_ztore",
        "mysql://user:secret@db.example.test:3306/erp_hzs",
        "mysql://user:secret@db.example.test:3306/other_database",
    ] {
        let error = database::parse_test_database_url(raw).unwrap_err();
        assert!(!error.contains("secret"));
        assert!(!error.contains("db.example.test"));
    }
}

#[test]
fn test_database_url_accepts_dedicated_test_database() {
    let parsed = database::parse_test_database_url(
        "mysql://test_user:test_password@127.0.0.1:3306/bz_ztore_test",
    )
    .unwrap();

    assert_eq!(parsed.path(), "/bz_ztore_test");
}

#[tokio::test]
async fn mysql_ci_service_responds_only_for_the_test_database() {
    if std::env::var_os("MYSQL_TEST_DATABASE_URL").is_none() {
        // Local runs need not have a MySQL daemon; CI provides this variable.
        return;
    }
    let url = database::test_database_url().expect("configured MySQL target must be safe");
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(1)
        .connect(url.as_str())
        .await
        .unwrap();
    let value: i32 = sqlx::query_scalar("SELECT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let name: String = sqlx::query_scalar("SELECT DATABASE()")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(value, 1);
    assert_eq!(name, "bz_ztore_test");
}

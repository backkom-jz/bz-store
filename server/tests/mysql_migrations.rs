#[test]
fn active_server_and_migrations_have_no_postgres_query_syntax() {
    fn visit(path: &std::path::Path, failures: &mut Vec<String>) {
        if path
            .file_name()
            .is_some_and(|name| name == "mysql_migrations.rs")
        {
            return;
        }
        if path.is_dir() {
            for entry in std::fs::read_dir(path).unwrap() {
                visit(&entry.unwrap().path(), failures);
            }
            return;
        }
        let Ok(source) = std::fs::read_to_string(path) else {
            return;
        };
        for forbidden in [
            "$1",
            "$2",
            "$3",
            "$4",
            "$5",
            "$6",
            "$7",
            "$8",
            "$9",
            "RETURNING",
            "ON CONFLICT",
            "to_jsonb(",
            "jsonb_build_object(",
            "jsonb_agg(",
            "::uuid",
            "TIMESTAMPTZ",
            "JSONB",
            "gen_random_uuid(",
            "current_schema(",
            "search_path",
            "FOR UPDATE OF",
            "DROP CONSTRAINT",
        ] {
            if source.contains(forbidden) {
                failures.push(format!("{} contains {forbidden}", path.display()));
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut failures = Vec::new();
    visit(&root.join("src"), &mut failures);
    visit(&root.join("tests"), &mut failures);
    visit(&root.join("migrations"), &mut failures);
    assert!(
        failures.is_empty(),
        "PostgreSQL syntax remains:\n{}",
        failures.join("\n")
    );
}

#[tokio::test]
async fn mysql_binary_uuid_round_trip() {
    let url = support::database::test_database_url().unwrap();
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(1)
        .connect(url.as_str())
        .await
        .unwrap();
    let expected = uuid::Uuid::new_v4();
    let actual: uuid::Uuid = sqlx::query_scalar("SELECT ?")
        .bind(expected)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(actual, expected);
}

#[tokio::test]
async fn mysql_migrations_create_core_constraints() {
    let url = support::database::test_database_url().unwrap();
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(1)
        .connect(url.as_str())
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let engine: String = sqlx::query_scalar(
        "SELECT ENGINE FROM information_schema.TABLES WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='orders'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(engine, "InnoDB");
}

#[tokio::test]
async fn mysql_integration_suite_preserves_purchase_invariants() {
    let app = support::TestApp::start().await;
    let admin = app.admin().await;
    let buyer = app.register().await;
    let course = app.course(&admin, 9900).await;
    app.payment_config().await;
    let order = app.order(&buyer, course.product).await;
    let proof = app.proof(&buyer, order).await;
    let approve_path = format!("/api/v1/admin/payments/{proof}/approve");
    let reference = format!("mysql-{}", uuid::Uuid::new_v4().simple());
    let requests = futures::future::join_all((0..8).map(|_| {
        app.request(
            "POST",
            &approve_path,
            serde_json::json!({"actual_amount_cents":9900,"payment_channel":"bank","transaction_reference":reference,"receipt_confirmed":true}),
            Some(&admin),
            &[],
        )
    }))
    .await;
    assert!(requests.iter().all(|response| response.status == 200));
    let entitlement_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_entitlements")
        .fetch_one(&app.state.pool)
        .await
        .unwrap();
    let audit_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM admin_audit_logs WHERE action='payment.approve'")
            .fetch_one(&app.state.pool)
            .await
            .unwrap();
    assert_eq!(entitlement_count, 1);
    assert_eq!(audit_count, 1);
}

mod support;

struct FakeEmailSender(bool);
#[async_trait::async_trait]
impl devstore_server::infrastructure::email::EmailSender for FakeEmailSender {
    async fn send(
        &self,
        _message: devstore_server::infrastructure::email::EmailMessage,
    ) -> Result<(), devstore_server::infrastructure::email::EmailError> {
        if self.0 {
            Ok(())
        } else {
            Err(devstore_server::infrastructure::email::EmailError::Delivery)
        }
    }
}

#[tokio::test]
async fn outbox_keeps_encrypted_message_after_smtp_failure_and_clears_after_success() {
    let app = support::TestApp::start().await;
    let email = format!("outbox-{}@example.test", uuid::Uuid::new_v4().simple());
    let _session = app.register_as(&email).await;
    let key = [17_u8; 32];
    let now = time::OffsetDateTime::now_utc() + time::Duration::seconds(4);
    let pending: (String, Vec<u8>, i16) = sqlx::query_as("SELECT status,encrypted_payload,attempts FROM email_outbox WHERE recipient=? ORDER BY created_at DESC LIMIT 1")
        .bind(&email).fetch_one(&app.state.pool).await.unwrap();
    assert_eq!(pending.0, "pending");
    let failed = FakeEmailSender(false);
    assert!(
        devstore_server::infrastructure::email::send_one(&app.state.pool, &key, &failed, now)
            .await
            .unwrap()
    );
    let after_failure: (String, Vec<u8>, i16) = sqlx::query_as("SELECT status,encrypted_payload,attempts FROM email_outbox WHERE recipient=? ORDER BY created_at DESC LIMIT 1")
        .bind(&email).fetch_one(&app.state.pool).await.unwrap();
    assert_eq!(after_failure.0, "pending");
    assert_eq!(after_failure.1, pending.1);
    assert_eq!(after_failure.2, 1);
    sqlx::query("UPDATE email_outbox SET available_at=DATE_SUB(UTC_TIMESTAMP(6),INTERVAL 1 SECOND) WHERE recipient=?").bind(&email).execute(&app.state.pool).await.unwrap();
    let delivered = FakeEmailSender(true);
    assert!(
        devstore_server::infrastructure::email::send_one(&app.state.pool, &key, &delivered, now)
            .await
            .unwrap()
    );
    let sent: (String, Vec<u8>) = sqlx::query_as("SELECT status,encrypted_payload FROM email_outbox WHERE recipient=? ORDER BY created_at DESC LIMIT 1")
        .bind(&email).fetch_one(&app.state.pool).await.unwrap();
    assert_eq!(sent.0, "sent");
    assert!(sent.1.is_empty());
}

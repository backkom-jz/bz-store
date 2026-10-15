#![allow(dead_code)]
#[path = "database.rs"]
pub mod database;
use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Request},
};
use devstore_server::{
    config::{Config, EmailConfig},
    state::AppState,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use tower::ServiceExt;
use uuid::Uuid;

pub struct TestApp {
    pub state: AppState,
    pub router: Router,
}
pub struct Course {
    pub product: Uuid,
    pub tutorial: Uuid,
    pub preview: Uuid,
    pub paid: Uuid,
    pub slug: String,
}
impl TestApp {
    pub async fn admin_totp_secret(&self, user: Uuid) -> Vec<u8> {
        use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
        let encrypted: Vec<u8> =
            sqlx::query_scalar("SELECT encrypted_secret FROM admin_mfa WHERE user_id=?")
                .bind(user)
                .fetch_one(&self.state.pool)
                .await
                .unwrap();
        let (nonce, ciphertext) = encrypted.split_at(12);
        Aes256Gcm::new_from_slice(&[23_u8; 32])
            .unwrap()
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .unwrap()
    }
    pub async fn email_token(&self, email: &str, kind: &str) -> String {
        use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
        let encrypted: Vec<u8> = sqlx::query_scalar("SELECT encrypted_payload FROM email_outbox WHERE recipient=? AND kind=? AND status='pending' ORDER BY created_at DESC,id DESC LIMIT 1")
            .bind(email.to_lowercase()).bind(kind).fetch_one(&self.state.pool).await.unwrap();
        let (nonce, ciphertext) = encrypted.split_at(12);
        let plaintext = Aes256Gcm::new_from_slice(&[17_u8; 32])
            .unwrap()
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .unwrap();
        let message: Value = serde_json::from_slice(&plaintext).unwrap();
        message["body"]
            .as_str()
            .unwrap()
            .split("token=")
            .nth(1)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .to_owned()
    }
    pub async fn proof(&self, buyer: &Session, order: Uuid) -> Uuid {
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(8, 8)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let r = self.upload(buyer, order, &bytes.into_inner()).await;
        assert_eq!(r.status, 201, "{}", r.body);
        Uuid::parse_str(r.body["data"]["id"].as_str().unwrap()).unwrap()
    }
    pub async fn order(&self, buyer: &Session, product: Uuid) -> Uuid {
        let r = self
            .request(
                "POST",
                "/api/v1/orders",
                json!({"product_id":product,"quantity":1}),
                Some(buyer),
                &[("idempotency-key", &Uuid::new_v4().to_string())],
            )
            .await;
        assert_eq!(r.status, 201, "{}", r.body);
        Uuid::parse_str(r.body["data"]["id"].as_str().unwrap()).unwrap()
    }
    pub async fn upload(&self, buyer: &Session, order: Uuid, bytes: &[u8]) -> TestResponse {
        let boundary = "devstore-test-boundary";
        let mut body=format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"receipt.png\"\r\nContent-Type: image/png\r\n\r\n").into_bytes();
        body.extend_from_slice(bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        self.raw(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/orders/{order}/payment-proof"))
                .header("origin", &self.state.config.site_url)
                .header("cookie", &buyer.cookie)
                .header("x-csrf-token", &buyer.csrf)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
    }
    pub async fn course(&self, admin: &Session, price: i64) -> Course {
        let slug = format!("course-{}", Uuid::new_v4().simple());
        let p = self
            .request(
                "POST",
                "/api/v1/admin/products",
                json!({"title":"测试教程","slug":slug,"price_cents":price,"type":"tutorial"}),
                Some(admin),
                &[],
            )
            .await;
        assert_eq!(p.status, 201, "{}", p.body);
        let product = Uuid::parse_str(p.body["data"]["id"].as_str().unwrap()).unwrap();
        let t = self
            .request(
                "POST",
                "/api/v1/admin/tutorials",
                json!({"title":"测试教程","slug":slug,"product_id":product}),
                Some(admin),
                &[],
            )
            .await;
        assert_eq!(t.status, 201, "{}", t.body);
        let tutorial = Uuid::parse_str(t.body["data"]["id"].as_str().unwrap()).unwrap();
        let mut chapters = vec![];
        for (preview, content) in [
            (true, "Public preview"),
            (false, "Paid secret: building a transactional store"),
        ] {
            let c=self.request("POST",&format!("/api/v1/admin/tutorials/{tutorial}/chapters"),json!({"title":if preview{"预览"}else{"付费章节"},"content_md":content,"is_preview":preview,"sort_order":if preview{0}else{1},"status":"published"}),Some(admin),&[]).await;
            assert_eq!(c.status, 201, "{}", c.body);
            chapters.push(Uuid::parse_str(c.body["data"]["id"].as_str().unwrap()).unwrap());
        }
        assert_eq!(
            self.request(
                "PATCH",
                &format!("/api/v1/admin/tutorials/{tutorial}"),
                json!({"status":"published"}),
                Some(admin),
                &[]
            )
            .await
            .status,
            200
        );
        assert_eq!(
            self.request(
                "PATCH",
                &format!("/api/v1/admin/products/{product}"),
                json!({"status":"published"}),
                Some(admin),
                &[]
            )
            .await
            .status,
            200
        );
        Course {
            product,
            tutorial,
            preview: chapters[0],
            paid: chapters[1],
            slug,
        }
    }
    pub async fn payment_config(&self) {
        sqlx::query("INSERT INTO site_settings(setting_key,value_json) VALUES('manual_payment',?) ON DUPLICATE KEY UPDATE value_json=VALUES(value_json)").bind(json!({"channel":"bank","payee":"TEST ONLY","account":"NOT A REAL ACCOUNT","instructions":"Integration test. Never transfer real money."})).execute(&self.state.pool).await.unwrap();
    }
}
pub struct TestResponse {
    pub status: u16,
    pub body: Value,
    pub headers: HeaderMap,
    pub bytes: bytes::Bytes,
}
#[derive(Clone)]
pub struct Session {
    pub cookie: String,
    pub csrf: String,
    pub id: Uuid,
}
impl TestApp {
    pub async fn start() -> Self {
        let mut config = Config::from_env().expect("test environment");
        config.email = Some(EmailConfig {
            smtp_host: "127.0.0.1".into(),
            smtp_port: 1025,
            smtp_username: String::new(),
            smtp_password: String::new(),
            smtp_tls: false,
            from: "DevStore Test <test@example.test>".into(),
            outbox_key: [17_u8; 32],
        });
        config.mfa_encryption_key = Some([23_u8; 32]);
        config.s3_bucket = std::env::var("S3_BUCKET").unwrap();
        devstore_server::infrastructure::storage::create_bucket(&config)
            .await
            .expect("test S3 bucket");
        let state = AppState::with_pool(config, isolated_pool().await)
            .await
            .unwrap();
        let _: String = redis::cmd("FLUSHDB")
            .query_async(&mut state.redis.clone())
            .await
            .unwrap();
        let router = devstore_server::build_router(state.clone());
        Self { state, router }
    }
    pub async fn request(
        &self,
        method: &str,
        path: &str,
        body: Value,
        session: Option<&Session>,
        headers: &[(&str, &str)],
    ) -> TestResponse {
        let mut builder = Request::builder()
            .method(method)
            .uri(path)
            .header("origin", &self.state.config.site_url)
            .header("content-type", "application/json");
        if let Some(s) = session {
            builder = builder
                .header("cookie", &s.cookie)
                .header("x-csrf-token", &s.csrf);
        }
        for (k, v) in headers {
            builder = builder.header(*k, *v);
        }
        self.raw(
            builder
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
    }
    pub async fn raw(&self, mut request: Request<Body>) -> TestResponse {
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(std::net::SocketAddr::from((
                [127, 0, 0, 1],
                40000,
            ))));
        let r = self.router.clone().oneshot(request).await.unwrap();
        let status = r.status().as_u16();
        let headers = r.headers().clone();
        let bytes = r.into_body().collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        TestResponse {
            status,
            body,
            headers,
            bytes,
        }
    }
    pub async fn get(&self, path: &str) -> TestResponse {
        self.request("GET", path, Value::Null, None, &[]).await
    }
    pub async fn register(&self) -> Session {
        let email = format!("{}@example.test", Uuid::new_v4().simple());
        self.register_as(&email).await
    }
    pub async fn register_as(&self, email: &str) -> Session {
        let r = self
            .request(
                "POST",
                "/api/v1/auth/register",
                json!({"email":email,"password":"a-long-password-123"}),
                None,
                &[],
            )
            .await;
        assert_eq!(r.status, 202, "{}", r.body);
        let user_id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email=?")
            .bind(email.to_lowercase())
            .fetch_one(&self.state.pool)
            .await
            .unwrap();
        // Existing business-flow fixtures act as if their test mailbox completed verification.
        sqlx::query("UPDATE users SET email_verified_at=UTC_TIMESTAMP(6) WHERE id=?")
            .bind(user_id)
            .execute(&self.state.pool)
            .await
            .unwrap();
        let login = self
            .request(
                "POST",
                "/api/v1/auth/login",
                json!({"email":email,"password":"a-long-password-123"}),
                None,
                &[],
            )
            .await;
        assert_eq!(login.status, 200, "{}", login.body);
        self.session_from(&login).await
    }
    pub async fn session_from(&self, r: &TestResponse) -> Session {
        let cookie = r
            .headers
            .get_all("set-cookie")
            .iter()
            .map(|h| h.to_str().unwrap().split(';').next().unwrap())
            .collect::<Vec<_>>()
            .join("; ");
        let mut s = Session {
            cookie,
            csrf: String::new(),
            id: Uuid::parse_str(r.body["data"]["id"].as_str().unwrap()).unwrap(),
        };
        let csrf = self
            .request("GET", "/api/v1/auth/csrf", Value::Null, Some(&s), &[])
            .await;
        assert_eq!(csrf.status, 200, "{}", csrf.body);
        s.csrf = csrf.body["data"]["csrf_token"]
            .as_str()
            .unwrap()
            .to_string();
        s
    }
    pub async fn admin(&self) -> Session {
        let email = format!("admin-{}@example.test", Uuid::new_v4().simple());
        let _old_session = self.register_as(&email).await;
        let id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email=?")
            .bind(&email)
            .fetch_one(&self.state.pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO user_roles(user_id,role_id) SELECT ?,id FROM roles WHERE code='admin'",
        )
        .bind(id)
        .execute(&self.state.pool)
        .await
        .unwrap();
        let (encoded, _) =
            devstore_server::infrastructure::mfa::begin(&self.state.pool, &self.state.config, id)
                .await
                .unwrap();
        let secret = data_encoding::BASE32_NOPAD
            .decode(encoded.as_bytes())
            .unwrap();
        let counter = time::OffsetDateTime::now_utc()
            .unix_timestamp()
            .div_euclid(30) as u64;
        let setup_code = devstore_server::infrastructure::mfa::totp_code(&secret, counter);
        devstore_server::infrastructure::mfa::confirm(
            &self.state.pool,
            &self.state.config,
            id,
            &setup_code,
            time::OffsetDateTime::now_utc(),
        )
        .await
        .unwrap();
        let code = devstore_server::infrastructure::mfa::totp_code(&secret, counter + 1);
        let login = self
            .request(
                "POST",
                "/api/v1/auth/login",
                json!({"email":email,"password":"a-long-password-123","totp_code":code}),
                None,
                &[],
            )
            .await;
        assert_eq!(login.status, 200, "{}", login.body);
        self.session_from(&login).await
    }
}

pub async fn isolated_pool() -> MySqlPool {
    let url = database::test_database_url()
        .expect("integration tests require MYSQL_TEST_DATABASE_URL targeting bz_ztore_test");
    let pool = MySqlPoolOptions::new()
        .max_connections(24)
        .after_connect(|connection, _| {
            Box::pin(async move {
                sqlx::query("SET time_zone = '+00:00'")
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(url.as_str())
        .await
        .expect("dedicated MySQL test database unavailable");
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let mut connection = pool.acquire().await.unwrap();
    sqlx::query("SET FOREIGN_KEY_CHECKS=0")
        .execute(&mut *connection)
        .await
        .unwrap();
    for table in [
        "admin_recovery_codes",
        "admin_mfa",
        "email_outbox",
        "auth_tokens",
        "download_logs",
        "ticket_messages",
        "service_tickets",
        "refunds",
        "blog_posts",
        "product_files",
        "payment_submissions",
        "file_objects",
        "user_entitlements",
        "order_requests",
        "order_items",
        "orders",
        "tutorial_chapters",
        "tutorials",
        "products",
        "categories",
        "admin_audit_logs",
        "user_sessions",
        "user_roles",
        "users",
        "site_settings",
    ] {
        sqlx::query(sqlx::AssertSqlSafe(format!("TRUNCATE TABLE `{table}`")))
            .execute(&mut *connection)
            .await
            .unwrap();
    }
    sqlx::query("SET FOREIGN_KEY_CHECKS=1")
        .execute(&mut *connection)
        .await
        .unwrap();
    drop(connection);
    pool
}

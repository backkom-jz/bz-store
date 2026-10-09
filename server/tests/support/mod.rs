#![allow(dead_code)]
use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Request},
};
use devstore_server::{config::Config, state::AppState};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::PgPoolOptions};
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
    pub async fn order(&self, buyer: &Session, product: Uuid) -> Uuid {
        let r=self.request("POST","/api/v1/orders",json!({"product_id":product,"quantity":1}),Some(buyer),&[("idempotency-key",&Uuid::new_v4().to_string())]).await;
        assert_eq!(r.status,201,"{}",r.body);
        Uuid::parse_str(r.body["data"]["id"].as_str().unwrap()).unwrap()
    }
    pub async fn upload(&self, buyer: &Session, order: Uuid, bytes: &[u8]) -> TestResponse {
        let boundary="devstore-test-boundary";
        let mut body=format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"receipt.png\"\r\nContent-Type: image/png\r\n\r\n").into_bytes();
        body.extend_from_slice(bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        self.raw(Request::builder().method("POST").uri(format!("/api/v1/orders/{order}/payment-proof")).header("origin",&self.state.config.site_url).header("cookie",&buyer.cookie).header("x-csrf-token",&buyer.csrf).header("content-type",format!("multipart/form-data; boundary={boundary}")).body(Body::from(body)).unwrap()).await
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
        sqlx::query("INSERT INTO site_settings(key,value_json) VALUES('manual_payment',$1) ON CONFLICT(key) DO UPDATE SET value_json=EXCLUDED.value_json").bind(json!({"channel":"bank","payee":"TEST ONLY","account":"NOT A REAL ACCOUNT","instructions":"Integration test. Never transfer real money."})).execute(&self.state.pool).await.unwrap();
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
        config.s3_bucket = std::env::var("S3_BUCKET").unwrap();
        devstore_server::infrastructure::storage::create_bucket(&config)
            .await
            .expect("test S3 bucket");
        let state = AppState::with_pool(config, isolated_pool().await)
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
        assert_eq!(r.status, 201, "{}", r.body);
        self.session_from(&r).await
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
        let s = self.register().await;
        sqlx::query(
            "INSERT INTO user_roles(user_id,role_id) SELECT $1,id FROM roles WHERE code='admin'",
        )
        .bind(s.id)
        .execute(&self.state.pool)
        .await
        .unwrap();
        s
    }
}

pub async fn isolated_pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .expect("run through bash scripts/test-server.sh with test services running");
    assert!(
        url.contains("devstore_test"),
        "integration tests must use the dedicated test database"
    );
    let root = PgPool::connect(&url)
        .await
        .expect("test PostgreSQL unavailable");
    let schema = format!("t_{}", Uuid::new_v4().simple());
    // Identifier is generated solely from UUID hex, never from a request.
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
        .execute(&root)
        .await
        .unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(24)
        .after_connect(move |conn, _| {
            let schema = schema.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path',$1,false)")
                    .bind(format!("{schema},public"))
                    .execute(conn)
                    .await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    root.close().await;
    pool
}

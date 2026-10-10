mod support;
use serde_json::Value;
use support::TestApp;
use uuid::Uuid;
#[test]
fn receipt_dimensions_and_truncation_are_bounded() {
    use devstore_server::modules::payments::upload::validate_receipt;
    for (w, h) in [(8001, 1), (5000, 4001)] {
        let mut output = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_luma8(w, h)
            .write_to(&mut output, image::ImageFormat::Png)
            .unwrap();
        assert_eq!(
            validate_receipt(&output.into_inner()).err().unwrap().status,
            422
        );
    }
    let bytes = receipt(image::ImageFormat::Png);
    assert_eq!(
        validate_receipt(&bytes[..bytes.len() / 2])
            .err()
            .unwrap()
            .status,
        422
    );
}
#[tokio::test]
async fn it_real_scanner_and_orphan_cleanup() {
    use devstore_server::{infrastructure::scanner::Scanner, jobs::cleanup_uploads::cleanup_batch};
    let mut app = TestApp::start().await;
    let scanner = app.state.scanner.clone();
    let eicar = br"X5O!P%@AP[4\PZX54(P^)7CC)7}$EICAR-STANDARD-ANTIVIRUS-TEST-FILE!$H+H*";
    assert_eq!(scanner.scan(eicar).await.unwrap_err().status, 422);
    let admin = app.admin().await;
    let buyer = app.register().await;
    let course = app.course(&admin, 9900).await;
    app.payment_config().await;
    let good = app.order(&buyer, course.product).await;
    let result = app
        .upload(&buyer, good, &receipt(image::ImageFormat::Png))
        .await;
    assert_eq!(result.status, 201);
    let orphan = app.order(&buyer, course.product).await;
    app.state.scanner = Scanner {
        addr: "127.0.0.1:1".into(),
    };
    app.router = devstore_server::build_router(app.state.clone());
    assert_eq!(
        app.upload(&buyer, orphan, &receipt(image::ImageFormat::Png))
            .await
            .status,
        503
    );
    sqlx::query("UPDATE file_objects SET created_at=DATE_SUB(UTC_TIMESTAMP(6),INTERVAL 2 DAY)")
        .execute(&app.state.pool)
        .await
        .unwrap();
    assert_eq!(
        cleanup_batch(
            &app.state,
            time::OffsetDateTime::now_utc() - time::Duration::days(1)
        )
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        cleanup_batch(
            &app.state,
            time::OffsetDateTime::now_utc() - time::Duration::days(1)
        )
        .await
        .unwrap(),
        0
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM file_objects")
        .fetch_one(&app.state.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let proof = result.body["data"]["id"].as_str().unwrap();
    assert_eq!(
        app.request(
            "GET",
            &format!("/api/v1/orders/{good}/payment-proof/{proof}"),
            Value::Null,
            Some(&buyer),
            &[]
        )
        .await
        .status,
        200
    );
}
fn receipt(format: image::ImageFormat) -> Vec<u8> {
    let mut data = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(8, 8)
        .write_to(&mut data, format)
        .unwrap();
    data.into_inner()
}
#[tokio::test]
async fn it_private_uploads_and_concurrent_submission() {
    let app = TestApp::start().await;
    let admin = app.admin().await;
    let buyer = app.register().await;
    let other = app.register().await;
    let course = app.course(&admin, 9900).await;
    app.payment_config().await;
    let order = app.order(&buyer, course.product).await;
    assert_eq!(
        app.upload(&other, order, &receipt(image::ImageFormat::Png))
            .await
            .status,
        404
    );
    assert_eq!(
        app.upload(&buyer, order, b"<script>fake png</script>")
            .await
            .status,
        422
    );
    assert_eq!(
        app.upload(&buyer, order, &vec![0; 5 * 1024 * 1024 + 1])
            .await
            .status,
        413
    );
    let data = receipt(image::ImageFormat::Png);
    let responses =
        futures::future::join_all((0..2).map(|_| app.upload(&buyer, order, &data))).await;
    assert_eq!(
        responses.iter().filter(|r| r.status == 201).count(),
        1,
        "{:?}",
        responses
            .iter()
            .map(|r| (&r.status, &r.body))
            .collect::<Vec<_>>()
    );
    assert_eq!(responses.iter().filter(|r| r.status == 409).count(), 1);
    let success = responses.iter().find(|r| r.status == 201).unwrap();
    let id = success.body["data"]["id"].as_str().unwrap();
    let path = format!("/api/v1/orders/{order}/payment-proof/{id}");
    let own = app
        .request("GET", &path, Value::Null, Some(&buyer), &[])
        .await;
    assert_eq!(own.status, 200);
    assert_eq!(own.bytes.as_ref(), data);
    assert_eq!(own.headers["content-type"], "image/png");
    assert!(
        own.headers["cache-control"]
            .to_str()
            .unwrap()
            .contains("no-store")
    );
    assert_eq!(
        app.request("GET", &path, Value::Null, Some(&other), &[])
            .await
            .status,
        404
    );
    assert_eq!(
        app.request(
            "GET",
            &format!("/api/v1/admin/payments/{id}/proof"),
            Value::Null,
            Some(&admin),
            &[]
        )
        .await
        .status,
        200
    );
    let key:String=sqlx::query_scalar("SELECT f.storage_key FROM file_objects f JOIN payment_submissions p ON p.file_id=f.id WHERE p.id=?").bind(Uuid::parse_str(id).unwrap()).fetch_one(&app.state.pool).await.unwrap();
    let anonymous = reqwest::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .get(format!(
            "{}/{}/{}",
            app.state.config.s3_endpoint, app.state.config.s3_bucket, key
        ))
        .send()
        .await
        .unwrap();
    assert!([401, 403, 404].contains(&anonymous.status().as_u16()));
    let next = app.order(&buyer, course.product).await;
    assert_eq!(
        app.upload(&buyer, next, &receipt(image::ImageFormat::Jpeg))
            .await
            .status,
        201
    );
}
#[tokio::test]
async fn it_scanner_failure_does_not_create_proof() {
    let mut app = TestApp::start().await;
    let admin = app.admin().await;
    let buyer = app.register().await;
    let course = app.course(&admin, 9900).await;
    app.payment_config().await;
    let order = app.order(&buyer, course.product).await;
    app.state.scanner.addr = "127.0.0.1:1".into();
    app.router = devstore_server::build_router(app.state.clone());
    let r = app
        .upload(&buyer, order, &receipt(image::ImageFormat::Png))
        .await;
    assert_eq!(r.status, 503, "{}", r.body);
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM payment_submissions")
        .fetch_one(&app.state.pool)
        .await
        .unwrap();
    assert_eq!(n, 0);
}

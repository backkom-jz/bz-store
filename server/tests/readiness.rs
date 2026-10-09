mod support;
#[tokio::test]
async fn it_ready_depends_on_all_services() {
    let app = support::TestApp::start().await;
    let r = app.get("/api/v1/health/ready").await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert_eq!(
        r.headers["x-request-id"].to_str().unwrap(),
        r.body["request_id"].as_str().unwrap()
    );
    app.state.pool.close().await;
    assert_eq!(app.get("/api/v1/health/ready").await.status, 503);
}

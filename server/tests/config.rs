use devstore_server::config::Config;
fn config() -> Config {
    Config {
        env: "production".into(),
        port: 8080,
        database_url: "postgres://test".into(),
        redis_url: "redis://test".into(),
        site_url: "http://example.test".into(),
        jwt_secret: "weak".into(),
        csrf_secret: "weak".into(),
        s3_endpoint: "http://objects".into(),
        s3_access_key: "key".into(),
        s3_secret_key: "secret".into(),
        s3_bucket: "bucket".into(),
        clamav_addr: "scanner:3310".into(),
        trusted_proxies: vec![],
    }
}
#[test]
fn production_rejects_insecure_configuration() {
    let mut c = config();
    assert!(
        c.validate().is_err(),
        "production must reject weak secrets and insecure origins"
    );
    c.jwt_secret = "a".repeat(40);
    c.csrf_secret = "b".repeat(40);
    c.site_url = "https://store.example".into();
    c.s3_endpoint = "https://s3.example".into();
    assert!(c.validate().is_ok());
    c.site_url = "https://store.example/other".into();
    assert!(c.validate().is_err());
}

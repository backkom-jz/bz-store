use devstore_server::config::Config;
fn config() -> Config {
    Config {
        env: "production".into(),
        port: 8080,
        database_url: "mysql://user:secret@rds.example.test:3306/bz_ztore".into(),
        redis_url: "redis://test".into(),
        site_url: "http://example.test".into(),
        jwt_secret: "weak".into(),
        csrf_secret: "weak".into(),
        s3_endpoint: "http://objects".into(),
        s3_access_key: "key".into(),
        s3_secret_key: "secret".into(),
        s3_bucket: "bucket".into(),
        trusted_proxies: vec![],
        email: None,
        mfa_encryption_key: Some([9_u8; 32]),
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
    c.site_url = "https://store.bengzi.cn".into();
    c.s3_endpoint = "https://s3.example".into();
    assert!(c.validate().is_ok());
    c.site_url = "https://store.bengzi.cn/other".into();
    assert!(c.validate().is_err());
}

#[test]
fn production_site_uses_the_configured_public_origin() {
    let mut config = config();
    config.jwt_secret = "a".repeat(40);
    config.csrf_secret = "b".repeat(40);
    config.site_url = "https://other.example".into();
    config.s3_endpoint = "https://objects.example".into();
    assert!(config.validate().is_err());

    config.site_url = "https://store.bengzi.cn".into();
    assert!(config.validate().is_ok());
}

#[test]
fn app_database_target_rejects_erp_and_test_databases_in_production() {
    let mut c = config();
    c.jwt_secret = "a".repeat(40);
    c.csrf_secret = "b".repeat(40);
    c.site_url = "https://store.bengzi.cn".into();
    c.s3_endpoint = "https://s3.example".into();
    assert!(c.validate().is_ok());
    c.database_url = "mysql://user:secret@rds.example.test:3306/erp_hzs".into();
    assert!(c.validate().is_err());
    c.database_url = "mysql://user:secret@rds.example.test:3306/bz_ztore_test".into();
    assert!(c.validate().is_err());
}

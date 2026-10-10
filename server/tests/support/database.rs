use url::Url;

const TEST_DATABASE_NAME: &str = "bz_ztore_test";

pub fn parse_test_database_url(raw: &str) -> Result<Url, String> {
    let url = Url::parse(raw).map_err(|_| "invalid MySQL test database URL".to_owned())?;
    if url.scheme() != "mysql" {
        return Err("test database URL must use the mysql scheme".to_owned());
    }
    if url.host_str().is_none() || url.username().is_empty() {
        return Err("test database URL must include a host and username".to_owned());
    }
    if url.path() != format!("/{TEST_DATABASE_NAME}") {
        return Err("database target is not the dedicated test database".to_owned());
    }
    Ok(url)
}

pub fn test_database_url() -> Result<Url, String> {
    let raw = std::env::var("MYSQL_TEST_DATABASE_URL")
        .map_err(|_| "MYSQL_TEST_DATABASE_URL is required".to_owned())?;
    parse_test_database_url(&raw)
}

use crate::common::{ApiResult, AppError};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    time::{Duration, timeout},
};
#[derive(Clone)]
pub struct Scanner {
    pub addr: String,
}
impl Scanner {
    pub async fn ping(&self) -> ApiResult<()> {
        timeout(Duration::from_secs(3), async {
            let mut stream = TcpStream::connect(&self.addr).await?;
            stream.write_all(b"nPING\n").await?;
            let mut line = String::new();
            BufReader::new(stream)
                .take(1024)
                .read_line(&mut line)
                .await?;
            if line.trim() != "PONG" {
                return Err(AppError::unavailable());
            }
            Ok(())
        })
        .await
        .map_err(|_| AppError::unavailable())?
    }
}

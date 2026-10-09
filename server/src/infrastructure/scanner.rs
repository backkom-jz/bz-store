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
    pub async fn scan(&self, bytes: &[u8]) -> ApiResult<()> {
        timeout(Duration::from_secs(30), async {
            let mut stream = TcpStream::connect(&self.addr).await?;
            stream.write_all(b"nINSTREAM\n").await?;
            for chunk in bytes.chunks(64 * 1024) {
                stream
                    .write_all(&(chunk.len() as u32).to_be_bytes())
                    .await?;
                stream.write_all(chunk).await?;
            }
            stream.write_all(&0u32.to_be_bytes()).await?;
            let mut reply = String::new();
            BufReader::new(stream)
                .take(1024)
                .read_line(&mut reply)
                .await?;
            match reply.trim() {
                "stream: OK" => Ok(()),
                value if value.ends_with(" FOUND") => Err(AppError::invalid("文件未通过安全扫描")),
                _ => Err(AppError::unavailable()),
            }
        })
        .await
        .map_err(|_| AppError::unavailable())?
    }
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

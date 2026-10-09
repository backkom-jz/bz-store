use clap::{Parser, Subcommand};
use devstore_server::{
    common::{ApiResult, AppError},
    config::Config,
    modules::auth::{dto::Credentials, service},
    state::AppState,
};
use std::io::Read;
#[derive(Parser)]
#[command(about = "DevStore 本地管理员工具")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Create {
        #[arg(long)]
        email: String,
        #[arg(long)]
        password_stdin: bool,
    },
    InitStorage,
    SeedDemo {
        #[arg(long)]
        admin_email: String,
    },
    PaymentConfig {
        #[arg(long)]
        admin_email: String,
        #[arg(long)]
        file: std::path::PathBuf,
    },
}
#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    if let Err(error) = run(Cli::parse()).await {
        eprintln!("{}: {}", error.code, error.message);
        std::process::exit(1);
    }
}
async fn run(cli: Cli) -> ApiResult<()> {
    let config = Config::from_env()?;
    if matches!(cli.command, Command::InitStorage) {
        devstore_server::infrastructure::storage::create_bucket(&config).await?;
        println!("Private storage initialized");
        return Ok(());
    }
    let state = AppState::connect(config).await?;
    state.migrate().await?;
    match cli.command {
        Command::Create {
            email,
            password_stdin,
        } => {
            let password = if password_stdin {
                let mut input = String::new();
                std::io::stdin().take(1024).read_to_string(&mut input)?;
                input.trim_end_matches(['\r', '\n']).to_owned()
            } else {
                rpassword::prompt_password("管理员密码（12–128 字符）: ")
                    .map_err(|_| AppError::invalid("无法读取密码"))?
            };
            let user = service::bootstrap_admin(&state, Credentials { email, password }).await?;
            println!("Admin created: {} ({})", user.email, user.id);
        }
        Command::InitStorage => unreachable!(),
        Command::SeedDemo { admin_email } => {
            let actor = operator(&state, &admin_email).await?;
            let product =
                devstore_server::modules::admin::operations::seed_demo(&state, &actor).await?;
            println!("Demo tutorial ready: {}", product["slug"]);
        }
        Command::PaymentConfig { admin_email, file } => {
            let actor = operator(&state, &admin_email).await?;
            let bytes = std::fs::read(file)?;
            let config = serde_json::from_slice(&bytes)
                .map_err(|_| AppError::invalid("付款配置 JSON 无效"))?;
            devstore_server::modules::admin::operations::payment_config(&state, &actor, config)
                .await?;
            println!("Payment configuration saved and audited");
        }
    }
    Ok(())
}
async fn operator(
    state: &AppState,
    email: &str,
) -> ApiResult<devstore_server::middleware::auth::CurrentUser> {
    let id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email=$1 AND status=1")
        .bind(email.trim().to_lowercase())
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(AppError::forbidden)?;
    let user = service::public_user(state, id).await?;
    if !user.is_admin {
        return Err(AppError::forbidden());
    }
    Ok(devstore_server::middleware::auth::CurrentUser {
        id,
        session_id: uuid::Uuid::nil(),
        is_admin: true,
    })
}

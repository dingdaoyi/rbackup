mod config;
mod commands;
mod backup;
mod scheduler;

use std::env;
use clap::Parser;
use tracing::{error, info};
use config::load_config;
use commands::{Cli, Commands};
use backup::backup_to;
use crate::config::{LogConfig};
use crate::scheduler::{clear_backup_schedule, schedule_backup};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let config_path = if cli.config.is_absolute() {
        cli.config.clone()
    } else {
        env::current_dir()?.join(cli.config)
    };

    let config = load_config(&config_path)?;
    tracing::debug!("读取配置文件成功");
    setup_logger(&config.log).await?;
    let default_server = config.get_default();
    match cli.command {
        Commands::Start { source, path, remote } => {
            let server = config.get(remote.as_ref()).or(default_server);
            match server {
                None => {
                    error!("未指定服务器");
                }
                Some(server) => {
                    info!("开始备份启动,备份文件:{:?},远程目录:{:?},远程服务:{:?}",source,path,remote);
                    backup_to(source, path, server).await?;
                }
            }
        }
        Commands::Schedule { cron, source, path, remote } => {
            let server = config.get(remote.as_ref()).or(default_server);
            match server {
                None => {
                    error!("未指定服务器");
                }
                Some(server) => {
                    let remote_path = path.unwrap_or_else(|| server.get_default_path());
                    schedule_backup(cron, source, remote_path, server, config_path)?;
                }
            }
        }
        Commands::Clear { path, all } => {
            clear_backup_schedule(path.as_deref(), all)?;
        }
    }
    Ok(())
}

async fn setup_logger(log_config: &LogConfig) -> Result<(), Box<dyn std::error::Error>> {
    env::set_var("RUST_LOG", &log_config.level);
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    Ok(())
}
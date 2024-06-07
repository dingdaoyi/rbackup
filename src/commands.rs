use std::path::PathBuf;
use clap::{Parser, Subcommand};

/// A fictional versioning CLI
#[derive(Debug, Parser)]
#[command(name = "rbackup")]
#[command(about = "备份命令", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    #[arg(short, long, default_value = "config.toml")]
    pub config: PathBuf,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// 开始备份
    #[command(arg_required_else_help = true)]
    Start {
        /// 配置文件远程地址名称,不配置使用配置文件默认配置
        #[arg(short, long, required = false)]
        remote: Option<String>,
        /// 远程目录,不配置使用配置文件默认配置
        #[arg(short, long, required = false)]
        path: Option<String>,
        /// 文件地址
        #[arg(short, long, required = true)]
        source: PathBuf,
    },
    /// 添加内容
    #[command(arg_required_else_help = true)]
    /// 设置定时任务
    #[command(arg_required_else_help = true)]
    Schedule {
        /// cron 表达式
        #[arg(short, long, required = true)]
        cron: String,
        /// 配置文件远程地址名称,不配置使用配置文件默认配置
        #[arg(short, long, required = false)]
        remote: Option<String>,
        /// 远程目录,不配置使用配置文件默认配置
        #[arg(short, long, required = false)]
        path: Option<String>,
        /// 文件地址
        #[arg(short, long, required = true)]
        source: PathBuf,
    },
    /// 清理定时任务
    #[command(arg_required_else_help = true)]
    Clear {
        /// 要清理的任务名称
        #[arg(short, long, required = false)]
        path: Option<String>,
        /// 清理所有任务
        #[arg(short, long, required = false)]
        all: bool,
    },
}

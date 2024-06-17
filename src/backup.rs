use std::fs::File;
use std::io;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_util::stream::{self, StreamExt};
use futures_util::TryStreamExt;
use glob::glob;
use indicatif::{ProgressBar, ProgressStyle};
use rusoto_core::{HttpClient, Region};
use rusoto_credential::StaticProvider;
use rusoto_s3::{PutObjectRequest, S3, S3Client};
use ssh2::Session;
use tokio::fs::read_dir;
use tokio::sync::Mutex;
use tokio_util::codec::{BytesCodec, FramedRead};
use tracing::{debug, error, info};

use crate::config::{S3Server, Server, SSHServer};

pub async fn collect_files(pattern: &PathBuf) -> Result<(bool, Vec<PathBuf>), Box<dyn std::error::Error>> {
    let mut paths = Vec::new();
    let mut is_dir = false;
    // 检查是否包含通配符
    if pattern.to_string_lossy().contains('*') {
        for entry in glob(pattern.to_str().unwrap())? {
            match entry {
                Ok(path) => {
                    if std::fs::metadata(&path)?.is_file() {
                        paths.push(path);
                    }
                }
                Err(e) => error!("{:?}", e),
            }
        }
    } else {
        let meta = std::fs::metadata(&pattern)?;
        if meta.is_dir() {
            info!("目录文件:{:?}",meta);
            is_dir = true;
            let mut dir = read_dir(pattern).await?;
            while let Some(entry) = dir.next_entry().await? {
                let path = entry.path();
                if std::fs::metadata(&path)?.is_file() {
                    paths.push(path);
                }
            }
        } else if meta.is_file() {
            paths.push(pattern.clone());
        }
    }

    Ok((is_dir, paths))
}

pub async fn backup_to(source: PathBuf, path: Option<String>, server: Server) -> Result<(), Box<dyn std::error::Error>> {
    let (is_dir, files) = collect_files(&source).await?;
    info!("选中文件:{:?}", files);
    for file in files {
        match &server {
            Server::S3(s3server) => {
                tracing::debug!("开始推送到s3服务器:{}",&s3server.name);
                backup_to_s3(file, path.clone().unwrap_or_else(|| s3server.default_path.clone()), s3server.clone(), is_dir).await?;
            }
            Server::SSH(ssh_server) => {
                debug!("开始推送到ssh服务器:{}",&ssh_server.name);
                backup_to_ssh(file, path.clone().unwrap_or_else(|| ssh_server.default_path.clone()), ssh_server.clone(), is_dir).await?;
            }
        };
    }

    Ok(())
}


pub async fn backup_to_s3(source: PathBuf, path: String, s3server: S3Server, is_dir: bool) -> Result<(), Box<dyn std::error::Error>> {
    let credentials = StaticProvider::new(s3server.access_key, s3server.secret_key, None, None);
    let region = match &s3server.endpoint {
        Some(endpoint) => Region::Custom {
            name: s3server.region.clone(),
            endpoint: endpoint.clone(),
        },
        None => s3server.region.parse::<Region>()?,
    };
    let client = S3Client::new_with(HttpClient::new()?, credentials, region);

    let metadata = tokio::fs::metadata(&source).await?;
    let total_size = metadata.len();
    let progress_bar = Arc::new(Mutex::new(ProgressBar::new(total_size)));
    {
        let progress_bar = progress_bar.clone();
        progress_bar.lock().await.set_style(
            ProgressStyle::default_bar()
                .template("{msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
                .unwrap()
                .progress_chars("#>-"),
        );
        progress_bar.lock().await.set_message("Uploading");
    }

    let file = tokio::fs::File::open(&source).await?;
    // 构造对象键
    let file_name = source.file_name().unwrap().to_str().unwrap();

    let mut destination_key = format!("{}/{}", path.trim_end_matches('/'), file_name);
    if is_dir {
        if let Some(parent) = source.parent() {
            let parent_dir = parent.file_name().unwrap().to_str().unwrap();
            destination_key = format!("{}/{}/{}", path.trim_end_matches('/'), parent_dir, file_name);
        }
    }

    debug!("远程路径:{}", destination_key);
    let file_stream = FramedRead::new(file, BytesCodec::new())
        .map_ok(|bytes| bytes.freeze())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));

    let body = stream::unfold((file_stream, progress_bar.clone()), move |(mut stream, progress_bar)| async {
        match stream.next().await {
            Some(Ok(chunk)) => {
                let len = chunk.len();
                progress_bar.lock().await.inc(len as u64);
                Some((Ok(chunk), (stream, progress_bar)))
            }
            Some(Err(e)) => Some((Err(e), (stream, progress_bar))),
            None => None,
        }
    });

    let put_request = PutObjectRequest {
        bucket: s3server.bucket,
        key: destination_key,
        body: Some(rusoto_s3::StreamingBody::new(body)),
        ..Default::default()
    };
    client.put_object(put_request).await?;
    progress_bar.lock().await.finish_with_message("Upload complete");
    info!("Backup completed successfully.");
    Ok(())
}

pub async fn backup_to_ssh(source: PathBuf, path: String, ssh_server: SSHServer, is_dir: bool) -> Result<(), Box<dyn std::error::Error>> {
    let tcp = TcpStream::connect(format!("{}:{}", ssh_server.server, ssh_server.port))?;
    let mut session = Session::new().unwrap();
    session.set_tcp_stream(tcp);
    session.handshake()?;
    session.userauth_password(&ssh_server.username, &ssh_server.password).unwrap();
    if !session.authenticated() {
        return Err(Box::new(io::Error::new(io::ErrorKind::PermissionDenied, "Authentication failed")));
    }

    let sftp = session.sftp().unwrap();
    let file_name = source.file_name().unwrap().to_str().unwrap();
    let mut remote_path = format!("{}/{}", path.trim_end_matches('/'), file_name);
    if is_dir {
        if let Some(parent) = source.parent() {
            let parent_dir = parent.file_name().unwrap().to_str().unwrap();
            remote_path = format!("{}/{}/{}", path.trim_end_matches('/'), parent_dir, file_name);
        }
    }
    debug!("上传文件地址:{}", remote_path);

    let remote_dir = Path::new(&remote_path).parent().unwrap();
    create_remote_directory_recursive(&sftp, remote_dir)?;
    let mut remote_file = sftp.create(Path::new(&remote_path)).unwrap();
    let metadata = tokio::fs::metadata(&source).await?;
    let total_size = metadata.len();
    let progress_bar = Arc::new(Mutex::new(ProgressBar::new(total_size)));
    {
        let progress_bar = progress_bar.clone();
        progress_bar.lock().await.set_style(
            ProgressStyle::default_bar()
                .template("{msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
                .unwrap()
                .progress_chars("#>-"),
        );
        progress_bar.lock().await.set_message("Uploading");
    }

    let local_file = File::open(&source);
    match local_file {
        Ok(mut local_file) => {
            let mut buffer = [0; 4096];
            let mut bytes_read = 0;
            while bytes_read < total_size {
                let bytes_to_read = std::cmp::min(buffer.len(), (total_size - bytes_read) as usize);
                let bytes_read_now = local_file.read(&mut buffer[..bytes_to_read])?;
                remote_file.write_all(&buffer[..bytes_read_now])?;
                bytes_read += bytes_read_now as u64;
                progress_bar.lock().await.inc(bytes_read_now as u64);
            }

            remote_file.flush()?;
            progress_bar.lock().await.finish_with_message("Upload complete");
            info!("Successfully uploaded {} to SSH server.", source.display());
            Ok(())
        }
        Err(msg) => {
            error!("错误:{}",msg);
            Err(Box::from(msg.to_string()))
        }
    }
}

// 递归创建远程目录
fn create_remote_directory_recursive(sftp: &ssh2::Sftp, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = dir.parent() {
        if !sftp.stat(parent).is_ok() {
            create_remote_directory_recursive(sftp, parent)?;
        }
    }
    match sftp.stat(dir) {
        Ok(_) => {
            Ok(())
        }
        Err(_) => {
            match sftp.mkdir(dir, 0o755) {
                Ok(_) => {
                    info!("Created directory: {}", dir.display());
                    Ok(())
                }
                Err(e) => {
                    let error_code = e.code();
                    if error_code == ssh2::ErrorCode::Session(-31) {
                        return Ok(());
                    }
                    return Err(Box::new(e));
                }
            }
        }
    }
}
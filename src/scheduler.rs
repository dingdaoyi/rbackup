use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;
use chrono::{Timelike, Utc};
use crate::config::Server;
use cron::Schedule;
use tracing::{error, info};


pub fn schedule_backup(
    cron: String,
    source: PathBuf,
    remote_path: String,
    server: Server,
    config_path: PathBuf
) -> Result<(), Box<dyn std::error::Error>> {
    let current_exe = std::env::current_exe()?;

    let command = format!(
        "{} start -c {} -s {} -p {} -r {} #rbackup_task",
        current_exe.display(),
        config_path.display(),
        source.display(),
        remote_path,
        server.get_name()
    );

    if cfg!(target_os = "windows") {
        // Windows 任务计划程序
        let schedule = Schedule::from_str(&cron)?;
        let next = schedule.upcoming(Utc).next().ok_or("No upcoming time found")?;
        let start_time = format!("{:02}:{:02}", next.hour(), next.minute());

        let output = Command::new("schtasks")
            .args(&[
                "/Create",
                "/SC", "DAILY",
                "/TN", "rbackup_task",
                "/TR", &command,
                "/ST", &start_time,
            ])
            .output()?;

        if !output.status.success() {
            error!("Failed to schedule task: {:?}", output);
        } else {
            error!("Successfully scheduled task: {:?}", output);
        }
    } else {
        // Linux 和 macOS 的 cron
        let cron_entry = format!("{} {}", cron, command);
        info!("Adding cron entry: {}", cron_entry);

        let output = Command::new("sh")
            .arg("-c")
            .arg(format!("(crontab -l 2>/dev/null; echo \"{}\") | crontab -", cron_entry))
            .output()?;

        if !output.status.success() {
            error!("Failed to schedule cron job: {:?}", output);
        } else {
            error!("Successfully scheduled cron job: {:?}", output);
        }
    }

    Ok(())
}

pub fn clear_backup_schedule(name: Option<&str>, clear_all: bool) -> Result<(), Box<dyn std::error::Error>> {
    if cfg!(target_os = "windows") {
        if clear_all {
            let output = Command::new("schtasks")
                .args(&["/Query", "/FO", "LIST"])
                .output()?;

            if !output.status.success() {
                error!("Failed to query tasks: {:?}", output);
                return Ok(());
            }

            let tasks = String::from_utf8_lossy(&output.stdout);
            for line in tasks.lines() {
                if line.starts_with("任务名称: ") && line.contains("rbackup_task") {
                    let task_name = line.split(": ").nth(1).unwrap();
                    let output = Command::new("schtasks")
                        .args(&["/Delete", "/TN", task_name, "/F"])
                        .output()?;

                    if !output.status.success() {
                        error!("Failed to delete task {}: {:?}", task_name, output);
                    }
                }
            }
        } else if let Some(task_name) = name {
            let output = Command::new("schtasks")
                .args(&["/Delete", "/TN", task_name, "/F"])
                .output()?;

            if !output.status.success() {
                error!("Failed to clear task {}: {:?}", task_name, output);
            }
        }
    } else {
        if clear_all {
            let output = Command::new("sh")
                .arg("-c")
                .arg("crontab -l")
                .output()?;

            if !output.status.success() {
                error!("Failed to list cron jobs: {:?}", output);
                return Ok(());
            }

            let cron_jobs = String::from_utf8_lossy(&output.stdout);
            let filtered_jobs: Vec<&str> = cron_jobs
                .lines()
                .filter(|line| !line.contains("#rbackup_task"))
                .collect();

            let new_cron_jobs = filtered_jobs.join("\n");
            let output = Command::new("sh")
                .arg("-c")
                .arg(format!("echo \"{}\" | crontab -", new_cron_jobs))
                .output()?;

            if !output.status.success() {
                error!("Failed to update cron jobs: {:?}", output);
            }
        } else if let Some(task_name) = name {
            let output = Command::new("sh")
                .arg("-c")
                .arg(format!("crontab -l | grep -v '{}' | crontab -", task_name))
                .output()?;

            if !output.status.success() {
                error!("Failed to clear cron job {}: {:?}", task_name, output);
            }
        }
    }
    Ok(())
}

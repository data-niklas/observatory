use std::process::Command;

use crate::model::{CheckedMonitoringTargetStatus, MonitoringTargetStatus};

pub async fn check_systemd_unit(unit: &str) -> CheckedMonitoringTargetStatus {
    let exit_status = Command::new("systemctl")
        .arg("is-active")
        .arg(unit)
        .arg("-q")
        .output()
        .expect("Failed to execute command")
        .status;

    if exit_status.success() {
        CheckedMonitoringTargetStatus {
            status: MonitoringTargetStatus::Healthy,
            description: "".to_string(),
        }
    } else {
        let output = Command::new("systemctl")
            .arg("status")
            .arg(unit)
            .output()
            .expect("Failed to execute command");
        let output = String::from_utf8(output.stdout).unwrap();
        CheckedMonitoringTargetStatus {
            status: MonitoringTargetStatus::Unhealthy,
            description: output,
        }
    }
}

pub async fn check_http_url(url: &str) -> CheckedMonitoringTargetStatus {
    let output = Command::new("curl")
        .arg("-s")
        .arg("-o")
        .arg("/dev/null")
        .arg("-w")
        .arg("%{http_code}")
        .arg(url)
        .output()
        .expect("Failed to execute command");
    let status_code = String::from_utf8(output.stdout).unwrap();
    let status_code = status_code.parse::<u16>().unwrap();
    let exit_status = output.status;

    if exit_status.success() {
        CheckedMonitoringTargetStatus {
            status: MonitoringTargetStatus::Healthy,
            description: "".to_string(),
        }
    } else {
        CheckedMonitoringTargetStatus {
            status: MonitoringTargetStatus::Unhealthy,
            description: format!("Status code: {}", status_code),
        }
    }
}

pub async fn check_fs_space(path: &str) -> CheckedMonitoringTargetStatus {
    let output = Command::new("df")
        .arg("--output=pcent")
        .arg(path)
        .output()
        .expect("Failed to execute command");
    let output = String::from_utf8(output.stdout).unwrap();
    // Percentage is in the second line, and includes a trailing % sign
    let percentage = output.lines().nth(1).unwrap().trim_end_matches('%').trim();
    let percentage = percentage.parse::<u8>().unwrap();

    let status = if percentage < 60 {
        MonitoringTargetStatus::Healthy
    } else if percentage < 90 {
        MonitoringTargetStatus::Degraded
    } else {
        MonitoringTargetStatus::Unhealthy
    };
    CheckedMonitoringTargetStatus {
        status,
        description: format!("Disk space usage: {}%", percentage),
    }
}

pub async fn check_ping(address: &str) -> CheckedMonitoringTargetStatus {
    let output = Command::new("ping")
        .arg("-c")
        .arg("1")
        .arg(address)
        .output()
        .expect("Failed to execute command");
    let ping = String::from_utf8(output.stdout).unwrap();
    let ping = ping
        .lines()
        .find(|line| line.starts_with("rtt"))
        .unwrap()
        .split('/')
        .nth(4)
        .unwrap()
        .parse::<f32>()
        .unwrap();
    // Format ping without decimal places
    let ping = ping.round() as u32;
    let exit_status = output.status;

    if exit_status.success() {
        CheckedMonitoringTargetStatus {
            status: MonitoringTargetStatus::Healthy,
            description: format!("{} ms", ping),
        }
    } else {
        CheckedMonitoringTargetStatus {
            status: MonitoringTargetStatus::Unhealthy,
            description: "".to_string(),
        }
    }
}

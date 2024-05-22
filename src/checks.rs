use crate::model::{CheckedMonitoringTargetStatus, MonitoringTargetStatus};
use dns_lookup::lookup_host;
use std::{process::Command, sync::Arc, time::Duration};
use systemstat::{Platform, System};

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

async fn check_http_url_result(url: &str) -> Result<CheckedMonitoringTargetStatus, reqwest::Error> {
    let client = reqwest::Client::new();
    let response = client.get(url).send().await?;
    let status_code = response.status().as_u16();

    if response.status().is_success() {
        Ok(CheckedMonitoringTargetStatus {
            status: MonitoringTargetStatus::Healthy,
            description: "".to_string(),
        })
    } else {
        Ok(CheckedMonitoringTargetStatus {
            status: MonitoringTargetStatus::Unhealthy,
            description: format!("Status code: {}", status_code),
        })
    }
}

pub async fn check_http_url(url: &str) -> CheckedMonitoringTargetStatus {
    match check_http_url_result(url).await {
        Ok(status) => status,
        Err(error) => CheckedMonitoringTargetStatus {
            status: MonitoringTargetStatus::Unhealthy,
            description: error.to_string(),
        },
    }
}

pub async fn check_fs_space(path: &str) -> CheckedMonitoringTargetStatus {
    let system = System::new();
    let percentage: u8 = match system.mount_at(path) {
        Ok(mount) => (100.0_f32 * (mount.total.as_u64() - mount.free.as_u64()) as f32
            / mount.total.as_u64() as f32)
            .round() as u8,
        Err(_) => {
            return CheckedMonitoringTargetStatus {
                status: MonitoringTargetStatus::Unhealthy,
                description: format!("Mount point not found: {}", path),
            }
        }
    };
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

async fn check_ping_result(
    address: &str,
) -> Result<CheckedMonitoringTargetStatus, ping_rs::PingError> {
    let ips = match lookup_host(address) {
        Ok(ips) => ips,
        Err(_) => {
            return Ok(CheckedMonitoringTargetStatus {
                status: MonitoringTargetStatus::Unhealthy,
                description: format!("Failed to resolve host: {}", address),
            });
        }
    };
    if ips.is_empty() {
        return Ok(CheckedMonitoringTargetStatus {
            status: MonitoringTargetStatus::Unhealthy,
            description: format!("No IP addresses found for host: {}", address),
        });
    }
    let addr = ips[0];
    let timeout = Duration::from_secs(1);
    let options = ping_rs::PingOptions {
        ttl: 128,
        dont_fragment: true,
    };
    let data = vec![];
    let data_ref = Arc::new(data.as_slice());
    let response = ping_rs::send_ping_async(&addr, timeout, data_ref, Some(&options)).await?;
    let ping = response.rtt;

    Ok(CheckedMonitoringTargetStatus {
        status: MonitoringTargetStatus::Healthy,
        description: format!("{} ms", ping),
    })
}

fn format_ping_error(ping_error: ping_rs::PingError) -> String {
    match ping_error {
        ping_rs::PingError::TimedOut => "Timed out".to_string(),
        ping_rs::PingError::IoPending => "IO pending".to_string(),
        ping_rs::PingError::OsError(a, text) => format!("OS error {}: {}", a, text),
        ping_rs::PingError::IpError(a) => format!("IP error: {}", a),
        ping_rs::PingError::BadParameter(a) => format!("Bad parameter: {}", a),
        ping_rs::PingError::DataSizeTooBig(a) => format!("Data size too big: {}", a),
    }
}

pub async fn check_ping(address: &str) -> CheckedMonitoringTargetStatus {
    match check_ping_result(address).await {
        Ok(status) => status,
        Err(error) => CheckedMonitoringTargetStatus {
            status: MonitoringTargetStatus::Unhealthy,
            description: format_ping_error(error),
        },
    }
}

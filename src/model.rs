use chrono::DateTime;

use chrono::Utc;
use rocket::serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub enum MonitoringTargetStatus {
    #[default]
    Healthy,
    Unhealthy,
    Degraded,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct ObservedMonitoringTargetStatus {
    pub timestamp: DateTime<Utc>,
    pub status: MonitoringTargetStatus,
    pub retries: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct Observation {
    pub observed_status: ObservedMonitoringTargetStatus,
    pub monitoring_target: MonitoringTargetDescriptor,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub enum MonitoringTargetTypeDescriptor {
    HTTP { url: String },
    Systemd { unit: String },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct MonitoringTargetDescriptor {
    pub name: String,
    pub interval: u64, // in seconds
    pub retries: u8,
    pub timeout: u64, // in seconds
    pub target: MonitoringTargetTypeDescriptor,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct MonitoringTarget {
    pub descriptor: MonitoringTargetDescriptor,
    pub status: Vec<ObservedMonitoringTargetStatus>,
}

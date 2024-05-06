use chrono::DateTime;

use chrono::Utc;
use rocket::serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
#[serde(crate = "rocket::serde")]
pub enum MonitoringTargetStatus {
    #[default]
    Healthy,
    Unhealthy,
    Degraded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct CheckedMonitoringTargetStatus {
    pub status: MonitoringTargetStatus,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct ObservedMonitoringTargetStatus {
    pub timestamp: DateTime<Utc>,
    pub status: MonitoringTargetStatus,
    pub description: String,
    pub retries: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde", tag = "type")]
pub enum MonitoringTargetTypeDescriptor {
    HTTP { url: String },
    Systemd { unit: String },
    Ping { target: String },
    FSSpace { path: String },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct MonitoringTargetDescriptor {
    pub id: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct Observation {
    pub observed_status: ObservedMonitoringTargetStatus,
    pub monitoring_target: MonitoringTargetDescriptor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde", tag = "type")]
pub enum Message {
    Observation(Observation),
    AppUpdate,
}

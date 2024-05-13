use std::time::Duration;

use chrono::Utc;
use rocket::tokio::sync::broadcast::Sender;
use rocket::{serde, tokio};

use crate::model::{
    CheckedMonitoringTargetStatus, Message, MonitoringTargetDescriptor, MonitoringTargetStatus,
    MonitoringTargetTypeDescriptor, Observation, ObservedMonitoringTargetStatus,
};
use crate::{args, checks::*, db};

async fn check_status(target: &MonitoringTargetDescriptor) -> CheckedMonitoringTargetStatus {
    match &target.target {
        MonitoringTargetTypeDescriptor::Systemd { unit } => check_systemd_unit(unit).await,
        MonitoringTargetTypeDescriptor::HTTP { url } => check_http_url(url).await,
        MonitoringTargetTypeDescriptor::Ping { target } => check_ping(target).await,
        MonitoringTargetTypeDescriptor::FSSpace { path } => check_fs_space(path).await,
    }
}

pub fn schedule_cleanup(args: &args::Args) {
    let db_path = args.database.clone();
    let observation_retention_duration = args.observation_retention_duration;
    let observation_retention_check_interval = args.observation_retention_check_interval;
    tokio::task::spawn(async move {
        let connection = db::init_db(&db_path).unwrap();
        loop {
            db::delete_old_observations(&connection, observation_retention_duration).unwrap();
            tokio::time::sleep(Duration::from_secs(observation_retention_check_interval)).await;
        }
    });
}

pub fn schedule_checks(event_sender: Sender<Message>, args: &args::Args) {
    let db_path = &args.database;
    let monitoring_targets = if args.config.is_none() {
        vec![]
    } else {
        let content = std::fs::read_to_string(&args.config.as_ref().unwrap()).unwrap();
        serde::json::from_str::<Vec<MonitoringTargetDescriptor>>(&content).unwrap()
    };
    let connection = db::init_db(db_path).unwrap();
    for target in monitoring_targets.iter() {
        db::create_or_update_monitoring_target(&connection, target).unwrap();
    }
    for target in monitoring_targets {
        let event_sender = event_sender.clone();
        let db_path = db_path.to_path_buf();
        tokio::task::spawn(async move {
            let connection = db::init_db(&db_path).unwrap();
            let mut tick = tokio::time::interval(Duration::from_secs(target.interval));
            loop {
                tick.tick().await;
                let mut retries_left = target.retries;
                let status = loop {
                    let status_awaitable = check_status(&target);
                    match tokio::time::timeout(
                        Duration::from_secs(target.timeout),
                        status_awaitable,
                    )
                    .await
                    {
                        Ok(observed_status) => {
                            if observed_status.status == MonitoringTargetStatus::Healthy {
                                break observed_status;
                            }
                            if retries_left == 0 {
                                break observed_status;
                            }
                            retries_left -= 1;
                        }
                        Err(_) => {
                            if retries_left == 0 {
                                break CheckedMonitoringTargetStatus {
                                    status: MonitoringTargetStatus::Unhealthy,
                                    description: "Timeout".to_string(),
                                };
                            }
                            retries_left -= 1;
                        }
                    }
                    tick.tick().await;
                };
                let retries = target.retries - retries_left;
                let observed_status = ObservedMonitoringTargetStatus {
                    timestamp: Utc::now(),
                    status: status.status,
                    description: status.description,
                    retries,
                };
                let observation = Observation {
                    monitoring_target: target.clone(),
                    observed_status,
                };
                db::add_observation(&connection, &observation).unwrap();
                let message = Message::Observation(observation);
                let _ = event_sender.send(message);
            }
        });
    }
}

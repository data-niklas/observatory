#[macro_use]
extern crate rocket;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use rocket::fs::{relative, FileServer};
use rocket::response::content::RawJson;
use rocket::response::stream::{Event, EventStream};
use rocket::serde::json::Json;
use rocket::tokio;
use rocket::tokio::select;
use rocket::tokio::sync::broadcast::{channel, error::RecvError, Sender};
use rocket::{Shutdown, State};

use chrono::Utc;

mod model;
use model::{
    MonitoringTargetDescriptor, MonitoringTargetStatus, MonitoringTargetTypeDescriptor,
    Observation, ObservedMonitoringTargetStatus,
};

mod db;

struct DBPath(pub PathBuf);
/// Returns an infinite stream of server-sent events. Each event is a message
/// pulled from a broadcast queue sent by the `post` handler.
#[get("/events")]
async fn events(queue: &State<Sender<Observation>>, mut end: Shutdown) -> EventStream![] {
    let mut rx = queue.subscribe();
    EventStream! {
        loop {
            let msg = select! {
                msg = rx.recv() => match msg {
                    Ok(msg) => msg,
                    Err(RecvError::Closed) => break,
                    Err(RecvError::Lagged(_)) => continue,
                },
                _ = &mut end => break,
            };

            yield Event::json(&msg);
        }
    }
}

#[get("/targets")]
async fn targets(db_path: &State<DBPath>) -> Json<Vec<MonitoringTargetDescriptor>> {
    let connection = db::init_db(&db_path.0).unwrap();
    let monitoring_targets = db::get_monitoring_target_descriptors(&connection).unwrap();
    Json(monitoring_targets)
}

#[get("/status/<name>")]
async fn status(
    name: &str,
    db_path: &State<DBPath>,
) -> Json<Option<ObservedMonitoringTargetStatus>> {
    let connection = db::init_db(&db_path.0).unwrap();
    let observed_statuses =
        db::get_last_observations(&connection, &vec![name.to_string()]).unwrap();
    let observed_statuses = observed_statuses.into_iter().next();
    Json(observed_statuses)
}

/// Receive a message from a form submission and broadcast it to any receivers.
// #[post("/message", data = "<form>")]
// fn post(form: Form<Message>, queue: &State<Sender<Message>>) {
//     // A send 'fails' if there are no active subscribers. That's okay.
//     let _ = queue.send(form.into_inner());
// }
async fn check_systemd_unit(unit: &str) -> MonitoringTargetStatus {
    let exit_status = Command::new("systemctl")
        .arg("is-active")
        .arg(unit)
        .arg("-q")
        .output()
        .expect("Failed to execute command")
        .status;

    if exit_status.success() {
        MonitoringTargetStatus::Healthy
    } else {
        MonitoringTargetStatus::Unhealthy
    }
}

async fn check_http_url(url: &str) -> MonitoringTargetStatus {
    let exit_status = Command::new("curl")
        .arg("-s")
        .arg("-o")
        .arg("/dev/null")
        .arg("-w")
        .arg("%{http_code}")
        .arg(url)
        .output()
        .expect("Failed to execute command")
        .status;

    if exit_status.success() {
        MonitoringTargetStatus::Healthy
    } else {
        MonitoringTargetStatus::Unhealthy
    }
}

async fn check_status(target: &MonitoringTargetDescriptor) -> MonitoringTargetStatus {
    match &target.target {
        MonitoringTargetTypeDescriptor::Systemd { unit } => check_systemd_unit(unit).await,
        MonitoringTargetTypeDescriptor::HTTP { url } => check_http_url(url).await,
        _ => MonitoringTargetStatus::Unhealthy,
    }
}

fn schedule_checks(event_sender: Sender<Observation>, db_path: &Path) {
    let monitoring_targets = vec![
        MonitoringTargetDescriptor {
            name: "vnstat".to_string(),
            interval: 5,
            retries: 0,
            timeout: 5,
            target: MonitoringTargetTypeDescriptor::Systemd {
                unit: "vnstat".to_string(),
            },
        },
        MonitoringTargetDescriptor {
            name: "google".to_string(),
            interval: 10,
            retries: 2,
            timeout: 5,
            target: MonitoringTargetTypeDescriptor::HTTP {
                url: "https://x.z.y".to_string(),
            },
        },
    ];

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
                        Ok(MonitoringTargetStatus::Healthy) => {
                            break MonitoringTargetStatus::Healthy;
                        }
                        _ => {
                            if retries_left == 0 {
                                break MonitoringTargetStatus::Unhealthy;
                            }
                            retries_left -= 1;
                        }
                    }
                    tick.tick().await;
                };
                let retries = target.retries - retries_left;
                let observed_status = ObservedMonitoringTargetStatus {
                    timestamp: Utc::now(),
                    status,
                    retries,
                };
                let observation = Observation {
                    monitoring_target: target.clone(),
                    observed_status,
                };
                db::add_observation(&connection, &observation).unwrap();
                let _ = event_sender.send(observation);
            }
        });
    }
}

#[rocket::main]
async fn main() {
    let db_path = PathBuf::from("monitoring.db");
    let event_stream = channel::<Observation>(1024);
    schedule_checks(event_stream.0.clone(), &db_path);
    let rocket = rocket::build()
        .manage(event_stream.0)
        .manage(DBPath(db_path))
        .mount("/", routes![events, targets, status])
        .mount("/", FileServer::from(relative!("static")));
    let ignited_rocket = rocket.ignite().await.expect("Rocket failed to ignite");
    let _finished_rocket = ignited_rocket
        .launch()
        .await
        .expect("Rocket failed to launch");
}

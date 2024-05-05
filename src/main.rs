#[macro_use]
extern crate rocket;

use std::process::Command;
use std::time::Duration;

use rocket::fs::{relative, FileServer};
use rocket::tokio::sync::broadcast::{channel, Sender};
use rocket::{serde, tokio, Config};

use chrono::Utc;

pub mod model;
use clap::Parser;
use model::{
    MonitoringTargetDescriptor, MonitoringTargetStatus, MonitoringTargetTypeDescriptor,
    Observation, ObservedMonitoringTargetStatus,
};

pub mod args;
pub mod db;
pub mod paths;

/// Returns an infinite stream of server-sent events. Each event is a message
/// pulled from a broadcast queue sent by the `post` handler.

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

async fn check_fs_space(path: &str) -> MonitoringTargetStatus {
    let output = Command::new("df")
        .arg("--output=pcent")
        .arg(path)
        .output()
        .expect("Failed to execute command");
    let output = String::from_utf8(output.stdout).unwrap();
    // Percentage is in the second line, and includes a trailing % sign
    let percentage = output.lines().nth(1).unwrap().trim_end_matches('%').trim();
    let percentage = percentage.parse::<u8>().unwrap();

    if percentage < 60 {
        MonitoringTargetStatus::Healthy
    } else if percentage < 90 {
        MonitoringTargetStatus::Degraded
    } else {
        MonitoringTargetStatus::Unhealthy
    }
}

async fn check_ping(address: &str) -> MonitoringTargetStatus {
    let exit_status = Command::new("ping")
        .arg("-c")
        .arg("1")
        .arg(address)
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
        MonitoringTargetTypeDescriptor::Ping { target } => check_ping(target).await,
        MonitoringTargetTypeDescriptor::FSSpace { path } => check_fs_space(path).await,
        _ => MonitoringTargetStatus::Unhealthy,
    }
}

fn schedule_checks(event_sender: Sender<Observation>, args: &args::Args) {
    let db_path = &args.database;
    let monitoring_targets = if args.targets.is_none() {
        vec![]
    } else {
        let content = std::fs::read_to_string(&args.targets.as_ref().unwrap()).unwrap();
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
    let args = args::Args::parse();
    let event_stream = channel::<Observation>(1024);
    schedule_checks(event_stream.0.clone(), &args);
    let rocket_config = Config::figment()
        .merge((Config::PORT, args.port))
        .merge((Config::ADDRESS, args.address.clone()));
    let rocket = rocket::build()
        .configure(rocket_config)
        .manage(event_stream.0)
        .manage(args)
        .mount("/", routes![paths::events, paths::targets, paths::status])
        .mount("/", FileServer::from(relative!("static")));
    let ignited_rocket = rocket.ignite().await.expect("Rocket failed to ignite");
    let _finished_rocket = ignited_rocket
        .launch()
        .await
        .expect("Rocket failed to launch");
}

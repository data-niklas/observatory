use crate::args::Args;
use crate::db::{get_last_observations, get_monitoring_target_descriptors, init_db, get_observations};
use crate::model::{MonitoringTargetDescriptor, Observation, ObservedMonitoringTargetStatus, Message};
use rocket::response::stream::{Event, EventStream};
use rocket::serde::json::Json;
use rocket::tokio::select;
use rocket::tokio::sync::broadcast::{error::RecvError, Sender};
use rocket::{Shutdown, State};

#[get("/events")]
pub async fn events(queue: &State<Sender<Message>>, mut end: Shutdown) -> EventStream![] {
    let mut rx = queue.subscribe();
    EventStream! {
        yield Event::json(&Message::AppUpdate);
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
pub async fn targets(args: &State<Args>) -> Json<Vec<MonitoringTargetDescriptor>> {
    let database = &args.database;
    let connection = init_db(database).unwrap();
    let monitoring_targets = get_monitoring_target_descriptors(&connection).unwrap();
    Json(monitoring_targets)
}

#[get("/status/<id>")]
pub async fn status(
    id: &str,
    args: &State<Args>,
) -> Json<Option<ObservedMonitoringTargetStatus>> {
    let database = &args.database;
    let connection = init_db(database).unwrap();
    let observed_statuses = get_last_observations(&connection, &vec![id.to_string()]).unwrap();
    let observed_statuses = observed_statuses.into_iter().next();
    Json(observed_statuses)
}

#[get("/observations/<id>")]
pub async fn observations(
    id: &str,
    args: &State<Args>,
) -> Json<Vec<Observation>> {
    let database = &args.database;
    let connection = init_db(database).unwrap();
    let observations = get_observations(&connection, id).unwrap();
    Json(observations)
}

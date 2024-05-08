#[macro_use]
extern crate rocket;

use rocket::figment::providers::Env;
use rocket::figment::Figment;
use rocket::fs::{relative, FileServer};
use rocket::tokio::sync::broadcast::channel;
use rocket::Config;

pub mod model;
use clap::Parser;
use model::Message;

pub mod args;
pub mod checks;
pub mod db;
pub mod paths;
pub mod schedule;

/// Returns an infinite stream of server-sent events. Each event is a message
/// pulled from a broadcast queue sent by the `post` handler.

/// Receive a message from a form submission and broadcast it to any receivers.
// #[post("/message", data = "<form>")]
// fn post(form: Form<Message>, queue: &State<Sender<Message>>) {
//     // A send 'fails' if there are no active subscribers. That's okay.
//     let _ = queue.send(form.into_inner());
// }

#[rocket::main]
async fn main() {
    let args = args::Args::parse();
    let event_stream = channel::<Message>(1024);
    schedule::schedule_cleanup(&args);
    schedule::schedule_checks(event_stream.0.clone(), &args);

    let mut rocket_config = Figment::from(Config::default())
        .merge(Env::prefixed("OBSERVATORY_").ignore(&["PROFILE"]).global());
    if args.port.is_some() {
        rocket_config = rocket_config.merge((Config::PORT, args.port.unwrap()));
    }
    if args.address.is_some() {
        rocket_config = rocket_config.merge((Config::ADDRESS, args.address.clone().unwrap()));
    }
    let rocket = rocket::build()
        .configure(rocket_config)
        .manage(event_stream.0)
        .manage(args)
        .mount(
            "/",
            routes![
                paths::events,
                paths::targets,
                paths::status,
                paths::observations
            ],
        )
        .mount("/", FileServer::from(relative!("static")));
    let ignited_rocket = rocket.ignite().await.expect("Rocket failed to ignite");
    let _finished_rocket = ignited_rocket
        .launch()
        .await
        .expect("Rocket failed to launch");
}

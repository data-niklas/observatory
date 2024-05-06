use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(short, long)]
    pub targets: Option<PathBuf>,

    #[arg(short, long, default_value = "monitoring.db")]
    pub database: PathBuf,

    #[arg(short, long)]
    pub port: Option<u16>,

    #[arg(short, long)]
    pub address: Option<String>,

}

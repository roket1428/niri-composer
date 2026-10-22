mod args;
mod client;
mod codec;
mod config;
mod daemon;
mod errors;
mod socket;

use std::error::Error;
use std::process::exit;

use miette::Result;
use tracing::error;

use crate::args::{Args, Commands};
use crate::client::dispatch;
use crate::config::Config;
use crate::daemon::run;

fn main() -> Result<()> {
    let args = Args::parse()?;

    match args.command {
        Some(command) => match command {
            Commands::Validate => {
                let (path, _) = Config::prepare(args.config)?;
                match Config::validate(&path) {
                    Ok(_) => {
                        println!("Config is valid");
                    }
                    Err(error) => return Err(error.into()),
                };
            }
            Commands::Daemon => {
                tracing_subscriber::fmt::init();
                if let Err(e) = run(args.config) {
                    error!(error = e.source(), "Daemon stopped due to fatal error: ");
                    exit(1);
                }
            }
            Commands::Dispatch(binding) => {
                dispatch(binding)?;
            }
        },
        None => {}
    }

    Ok(())
}

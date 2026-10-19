#![allow(unused_assignments)]

mod args;
mod codec;
mod config;
mod daemon;
mod socket;

use miette::{Context, Result, miette};

use crate::args::{Args, Commands};
use crate::config::Config;
use crate::daemon::daemon;

fn main() -> Result<()> {
    let args = Args::parse()
        .map_err(|e| miette!("{e}"))
        .wrap_err("Failed to parse arguments")?;

    match args.command {
        Some(command) => match command {
            Commands::Validate => {
                let (path, _) = Config::prepare(args.config)?;
                Config::validate(&path)?;
            }
            Commands::Daemon => {
                let (path, dir) = Config::prepare(args.config)?;
                let config = Config::validate(&path)?;
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(daemon(dir, config))?;
            }
        },
        None => {}
    }

    Ok(())
}

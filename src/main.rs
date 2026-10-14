mod args;
mod codec;
mod config;
mod daemon;
mod socket;

use anyhow::Error;

use crate::args::{Args, Commands};
use crate::config::validate;
use crate::daemon::daemon;

fn main() -> Result<(), Error> {
    let args = Args::parse()?;

    match &args.command {
        Some(command) => match command {
            Commands::Validate => {
                validate(&args.config);
            }
            Commands::Daemon => {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(daemon(&args.config))?;
            }
        },
        None => {}
    }

    Ok(())
}

use std::{path::PathBuf, process::exit, str::FromStr};

use lexopt::{Error, Parser};

#[rustfmt::skip]
const HELP: &str =
r#"A composer daemon for niri wayland compositor

Usage: niri-composer <COMMAND> [OPTIONS]

Commands:
  daemon    Start the daemon
  validate  Validate the config

Options:
  -c, --config <PATH>    Path to config file
  -h, --help             Print help
  -V, --version          Print version
"#;

pub enum Commands {
    Daemon,
    Validate,
}

impl FromStr for Commands {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        match s {
            "daemon" => Ok(Self::Daemon),
            "validate" => Ok(Self::Validate),
            other => Err(format!("unknown subcommand: '{}'", other).into()),
        }
    }
}

pub struct Args {
    pub command: Option<Commands>,
    pub config: Option<PathBuf>,
}

impl Args {
    pub fn parse() -> Result<Self, Error> {
        use lexopt::prelude::*;

        let mut command: Option<Commands> = None;
        let mut config: Option<PathBuf> = None;
        let mut parser = Parser::from_env();
        while let Some(arg) = parser.next()? {
            match arg {
                Short('V') | Long("version") => {
                    println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
                    exit(0);
                }
                Short('h') | Long("help") => {
                    print!("{}", HELP);
                    exit(0);
                }
                Short('c') | Long("config") => {
                    config = Some(parser.value()?.parse()?);
                }
                Value(maybe_command) => {
                    command = Some(maybe_command.parse()?);
                }
                _ => return Err(arg.unexpected()),
            }
        }

        Ok(Args { command, config })
    }
}

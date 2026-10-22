use std::{path::PathBuf, process::exit};

use lexopt::Arg::{Long, Short, Value};
use lexopt::Parser;

use crate::config::Config;
use crate::errors::ArgError;

#[rustfmt::skip]
const HELP: &'static str =
r#"A composer daemon for niri wayland compositor

Usage: niri-composer <COMMAND> [OPTIONS]

Commands:
  daemon             Start the daemon
  validate           Validate the config
  dispatch <NAME>    Trigger the target dispatch

Options:
  -c, --config <PATH>    Path to config file
  -h, --help             Print help
  -V, --version          Print version
"#;

pub enum Commands {
    Daemon,
    Validate,
    Dispatch(String),
}

pub struct Args {
    pub command: Option<Commands>,
    pub config: Option<PathBuf>,
}

impl Args {
    pub fn parse() -> Result<Self, ArgError> {
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
                    let path = match parser.value()?.into_string() {
                        Ok(path) => path,
                        Err(arg) => {
                            return Err(ArgError::NotUnicode { arg });
                        }
                    };

                    match Config::validate_path(path) {
                        Ok(path) => {
                            config = Some(path);
                        }
                        Err(error) => {
                            return Err(ArgError::from(error));
                        }
                    };
                }
                Value(maybe_command) => {
                    if command.is_none() {
                        let maybe_command = match maybe_command.into_string() {
                            Ok(maybe_command) => maybe_command,
                            Err(arg) => {
                                return Err(ArgError::NotUnicode { arg });
                            }
                        };

                        match maybe_command.as_str() {
                            "validate" => {
                                command = Some(Commands::Validate);
                            }
                            "daemon" => {
                                command = Some(Commands::Daemon);
                            }
                            "dispatch" => {
                                let binding = match parser.value()?.into_string() {
                                    Ok(binding) => binding,
                                    Err(arg) => {
                                        return Err(ArgError::NotUnicode { arg });
                                    }
                                };

                                match Config::validate_binding(&binding) {
                                    Ok(()) => {
                                        command = Some(Commands::Dispatch(binding));
                                    }
                                    Err(error) => {
                                        return Err(ArgError::from(error));
                                    }
                                }
                            }
                            _ => {
                                return Err(ArgError::UnknownSubcommand { maybe_command });
                            }
                        }
                    } else {
                        return Err(ArgError::UnexpectedSubcommand { maybe_command });
                    }
                }
                _ => {
                    return Err(ArgError::LexoptError(arg.unexpected()));
                }
            }
        }

        if command.is_none() {
            print!("{}", HELP);
            exit(0);
        }

        Ok(Args { command, config })
    }
}

#![allow(unused)]

use std::{fs, path::PathBuf, path::absolute};

use directories::ProjectDirs;
use knuffel::Decode;
use knuffel::ast::SpannedNode;
use knuffel::errors::DecodeError;
use knuffel::parse;
use knuffel::traits::ErrorSpan;

use miette::{Context, IntoDiagnostic, Result, bail};

type NiriActions = Vec<niri_config::Action>;
type Conditions = Vec<Condition>;

#[derive(Decode, Debug, Default, Clone)]
pub struct Config {
    #[knuffel(children(name = "dispatch"))]
    pub dispatches: Vec<Dispatch>,
}

impl Config {
    pub const FILE: &'static str = "config.kdl";

    /// max binding name length
    const MAX_NAME_LEN: usize = 32;

    fn get_config_file() -> Result<PathBuf> {
        let mut path = match ProjectDirs::from("", "", env!("CARGO_PKG_NAME")) {
            Some(dirs) => dirs.config_dir().to_owned(),
            None => {
                bail!("couldn't find the config directory because home is not set")
            }
        };
        path.push(Self::FILE);
        Ok(path)
    }

    pub fn prepare(path: Option<PathBuf>) -> Result<(PathBuf, PathBuf)> {
        let path = match path {
            Some(p) => absolute(p).into_diagnostic()?,
            None => Self::get_config_file()?,
        };

        let dir = match path.parent() {
            Some(dir) => dir.to_owned(),
            None => {
                bail!("couldn't retrieve the parent directory")
            }
        };

        Ok((path, dir))
    }

    pub fn validate(path: &PathBuf) -> Result<Self> {
        let config = parse::<Self>(
            Self::FILE,
            fs::read_to_string(path)
                .into_diagnostic()
                .wrap_err("Config validation failed!")?
                .as_str(),
        )
        .wrap_err("Config validation failed!")?;
        println!("Config is valid");
        Ok(config)
    }
}

#[derive(Debug, Clone)]
pub struct Dispatch {
    pub name: String,
    expression: Expression,
    fallback: Option<NiriActions>,
}

impl<S: ErrorSpan> Decode<S> for Dispatch {
    fn decode_node(
        node: &SpannedNode<S>,
        ctx: &mut knuffel::decode::Context<S>,
    ) -> Result<Self, DecodeError<S>> {
        let raw = RawDispatch::decode_node(node, ctx)?;

        if raw.name.len() > Config::MAX_NAME_LEN {
            return Err(DecodeError::unsupported(
                node,
                format!(
                    "dispatch name can be at most {} bytes long (got {})",
                    Config::MAX_NAME_LEN,
                    raw.name.len()
                ),
            ));
        }

        let branches = raw.branches.len() >= 2;
        let expression = match (branches, raw.when, raw.action) {
            (true, None, None) => Expression::Branches(raw.branches),
            (false, Some(when), Some(action)) => Expression::Inline { when, action },
            (false, None, None) => {
                return Err(DecodeError::unsupported(
                    node,
                    "either n >= 2 branches or a single when+action pair must be defined",
                ));
            }
            (false, None, _) => {
                return Err(DecodeError::unsupported(
                    node,
                    "child node `when` is required",
                ));
            }
            (false, _, None) => {
                return Err(DecodeError::unsupported(
                    node,
                    "child node `action` is required",
                ));
            }
            (true, _, _) => {
                return Err(DecodeError::unsupported(
                    node,
                    "branches and inline when+action nodes are mutually exclusive (you can't have both)",
                ));
            }
        };

        Ok(Dispatch {
            name: raw.name,
            expression,
            fallback: raw.fallback,
        })
    }
}

#[derive(Decode, Debug)]
struct RawDispatch {
    #[knuffel(argument)]
    name: String,

    #[knuffel(children(name = "branch"))]
    branches: Vec<Branch>,

    #[knuffel(child, unwrap(children))]
    when: Option<Conditions>,

    #[knuffel(child, unwrap(children))]
    action: Option<NiriActions>,

    #[knuffel(child, unwrap(children))]
    fallback: Option<NiriActions>,
}

#[derive(Debug, Clone)]
enum Expression {
    Branches(Vec<Branch>),
    Inline {
        when: Conditions,
        action: NiriActions,
    },
}

#[derive(Decode, Debug, Clone)]
struct Branch {
    #[knuffel(child, unwrap(children))]
    when: Conditions,

    #[knuffel(child, unwrap(children))]
    action: NiriActions,
}

#[derive(Decode, Debug, Clone)]
enum Condition {
    All(#[knuffel(children)] Vec<NestedCondition>),
    Any(#[knuffel(children)] Vec<NestedCondition>),
    Not(#[knuffel(children)] Vec<NestedCondition>),

    Overview(#[knuffel(argument, default = true)] bool),
    ConfigFailed(#[knuffel(argument, default = true)] bool),
}

#[derive(Decode, Debug, Clone)]
enum NestedCondition {
    Overview(#[knuffel(argument, default = true)] bool),
    ConfigFailed(#[knuffel(argument, default = true)] bool),
}

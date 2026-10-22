use std::{
    fmt::Debug,
    fs,
    path::{PathBuf, absolute},
};

use directories::ProjectDirs;
use knuffel::Decode;
use knuffel::ast::SpannedNode;
use knuffel::errors::DecodeError;
use knuffel::parse;
use knuffel::traits::ErrorSpan;

use crate::errors::{
    BindingParseError, ConfigParseError, ConfigPrepareError, ConfigRetrivalError, PathParseError,
};

type NiriActions = Vec<niri_config::Action>;
type Conditions = Vec<Condition>;

#[derive(Decode, Debug, Default, Clone)]
pub struct Config {
    #[knuffel(children(name = "dispatch"))]
    pub dispatches: Vec<Dispatch>,
}

impl Config {
    pub const FILE: &'static str = "config.kdl";

    fn get_config_file() -> Result<PathBuf, ConfigRetrivalError> {
        let mut path = match ProjectDirs::from("", "", env!("CARGO_PKG_NAME")) {
            Some(dirs) => dirs.config_dir().to_owned(),
            None => {
                return Err(ConfigRetrivalError::HomeNotSet);
            }
        };
        path.push(Self::FILE);
        Ok(path)
    }

    pub fn prepare(path: Option<PathBuf>) -> Result<(PathBuf, PathBuf), ConfigPrepareError> {
        let path = match path {
            Some(p) => absolute(p)?,
            None => Self::get_config_file()?,
        };

        let dir = match path.parent() {
            Some(dir) => dir.to_owned(),
            None => {
                return Err(ConfigPrepareError::ParentNotFound);
            }
        };

        Ok((path, dir))
    }

    pub fn validate(path: &PathBuf) -> Result<Self, ConfigParseError> {
        parse::<Self>(Self::FILE, fs::read_to_string(path)?.as_str())
            .map_err(|e| ConfigParseError::from(e))
    }

    /// max binding name length
    const MAX_NAME_LEN: usize = 32;

    pub fn validate_binding(name: &str) -> Result<(), BindingParseError> {
        if name.is_empty() {
            return Err(BindingParseError::BindingEmpty);
        }

        if name.len() > Self::MAX_NAME_LEN {
            return Err(BindingParseError::MaxLengthExceeded {
                e: Self::MAX_NAME_LEN,
                g: name.len(),
            });
        }

        if !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return Err(BindingParseError::Invalid);
        }

        Ok(())
    }

    pub fn validate_path(path: String) -> Result<PathBuf, PathParseError> {
        if path.is_empty() {
            return Err(PathParseError::PathEmpty);
        }

        let path = PathBuf::from(path);
        if !path.is_file() {
            return Err(PathParseError::NotAFile);
        }

        Ok(path)
    }
}

#[allow(unused)]
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

        if let Err(e) = Config::validate_binding(&raw.name) {
            return Err(DecodeError::unsupported(node, e.to_string()));
        }

        let branches = raw.branches.len() >= 2;
        let expression = match (branches, raw.when, raw.action) {
            (true, None, None) => Expression::Branches(raw.branches),
            (false, Some(when), Some(action)) => Expression::Inline { when, action },
            (false, None, None) => {
                return Err(DecodeError::unsupported(
                    node,
                    "Either n >= 2 branches or a single when+action pair must be defined",
                ));
            }
            (false, None, _) => {
                return Err(DecodeError::unsupported(
                    node,
                    "Child node `when` is required",
                ));
            }
            (false, _, None) => {
                return Err(DecodeError::unsupported(
                    node,
                    "Child node `action` is required",
                ));
            }
            (true, _, _) => {
                return Err(DecodeError::unsupported(
                    node,
                    "Branches and inline when+action nodes are mutually exclusive (you can't have both)",
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

#[allow(unused)]
#[derive(Debug, Clone)]
enum Expression {
    Branches(Vec<Branch>),
    Inline {
        when: Conditions,
        action: NiriActions,
    },
}

#[allow(unused)]
#[derive(Decode, Debug, Clone)]
struct Branch {
    #[knuffel(child, unwrap(children))]
    when: Conditions,

    #[knuffel(child, unwrap(children))]
    action: NiriActions,
}

#[allow(unused)]
#[derive(Decode, Debug, Clone)]
enum Condition {
    All(#[knuffel(children)] Vec<NestedCondition>),
    Any(#[knuffel(children)] Vec<NestedCondition>),
    Not(#[knuffel(children)] Vec<NestedCondition>),

    Overview(#[knuffel(argument, default = true)] bool),
    ConfigFailed(#[knuffel(argument, default = true)] bool),
}

#[allow(unused)]
#[derive(Decode, Debug, Clone)]
enum NestedCondition {
    Overview(#[knuffel(argument, default = true)] bool),
    ConfigFailed(#[knuffel(argument, default = true)] bool),
}

use error_set::error_set;
use miette::Diagnostic;
use std::ffi::OsString;

error_set! {
    #[derive(Diagnostic)]
    ArgError := {
        #[display("Argument '{:?}' is not unicode", arg)]
        NotUnicode { arg: OsString },

        #[display("Unknown subcommand: '{maybe_command}'")]
        UnknownSubcommand { maybe_command: String },

        #[display("Unexpected subcommand: '{:?}'", maybe_command)]
        UnexpectedSubcommand { maybe_command: OsString },

        #[display("Failed to parse arguments")]
        LexoptError(lexopt::Error)
    } || PathParseError || BindingParseError

    PathParseError := {
        #[display("Config path cannot be empty")]
        PathEmpty,

        #[display("Config path is not a file")]
        NotAFile,
    }

    BindingParseError := {
        #[display("Binding name cannot be empty")]
        BindingEmpty,

        #[display("Max binding name length exceeded (expected: {e}, got: {g})")]
        MaxLengthExceeded { e: usize, g: usize },

        #[display("Binding name can only include a-Z, 0-9 and '-'")]
        Invalid
    }

    /// user facing
    #[derive(Diagnostic)]
    ConfigParseError := {
        #[diagnostic(transparent)]
        #[display("Config validation failed!")]
        KnuffelError(knuffel::errors::Error),

        #[display("Config validation failed!")]
        IoError(std::io::Error)
    }

    /// user facing
    #[derive(Diagnostic)]
    ConfigPrepareError := {
        #[display("Couldn't retrieve the parent directory")]
        ParentNotFound,
        IoError(std::io::Error)
    } || ConfigRetrivalError

    /// user facing
    ConfigRetrivalError := {
        #[display("Couldn't retrieve the config directory: $HOME is not set")]
        HomeNotSet
    }

    /// user facing
    #[derive(Diagnostic)]
    ClientError := {
        #[display("{context}")]
        IoErrorCtx(std::io::Error) {
            context: &'static str
        }
    }

    DaemonError := {
        (ConfigParseError),
        (ConfigPrepareError),
    } || ServerError || LockError

    ServerError := {
        JoinError(tokio::task::JoinError)
    } || TaskError

    TaskError := {
        #[display("'{channel}' channel is closed")]
        ChannelClosed { channel: &'static str },

        (SocketError),
        (notify::Error),
        IoError(std::io::Error)
    }

    LockError := {
        #[display("Another daemon instance is running")]
        AlreadyRunning,

        #[display("Failed to acquire lock")]
        TryLockError(std::fs::TryLockError),

        #[display("{context}")]
        IoErrorCtx(std::io::Error) {
            context: &'static str
        }
    }

    SocketError := {
        #[display("{path} is not set", path = niri_ipc::socket::SOCKET_PATH_ENV)]
        PathNotSet,

        #[display("Event stream request is denied by niri")]
        EventStreamDenied,

        #[display("EOF while reading the stream")]
        EOF,

        #[display("Niri error: {msg}")]
        NiriError { msg: String },

        CodecError(CodecError),

        #[display("{context}")]
        IoErrorCtx(std::io::Error) {
            context: &'static str
        }
    }

    CodecError := {
        #[display("Max frame length exceeded")]
        MaxFrameLengthExceeded,

        SerdeJsonError(serde_json::Error),
        IoError(std::io::Error)
    }
}

pub struct Ctx<E> {
    pub source: E,
    pub context: &'static str,
}

pub trait Context<T, E>: Sized + Into<Result<T, E>> {
    fn context(self, context: &'static str) -> Result<T, Ctx<E>> {
        self.into().map_err(|source| Ctx { source, context })
    }
}

impl<T, E> Context<T, E> for Result<T, E> {}

impl From<std::io::Error> for LockError {
    fn from(source: std::io::Error) -> Self {
        Self::IoErrorCtx {
            source,
            context: "Failed to acquire lock",
        }
    }
}

impl From<Ctx<std::io::Error>> for LockError {
    fn from(value: Ctx<std::io::Error>) -> Self {
        Self::IoErrorCtx {
            source: value.source,
            context: value.context,
        }
    }
}

impl From<Ctx<std::io::Error>> for SocketError {
    fn from(value: Ctx<std::io::Error>) -> Self {
        Self::IoErrorCtx {
            source: value.source,
            context: value.context,
        }
    }
}

impl From<Ctx<std::io::Error>> for ClientError {
    fn from(value: Ctx<std::io::Error>) -> Self {
        Self::IoErrorCtx {
            source: value.source,
            context: value.context,
        }
    }
}

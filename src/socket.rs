use std::env;

use miette::Diagnostic;

use bytes::BytesMut;
use niri_ipc::{Event, Reply, Request, Response, socket::SOCKET_PATH_ENV};

use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::codec::{Codec, CodecError};

const SOCKET_BUF_SIZE: usize = 2048;
const MAX_FRAME_LENGTH: usize = 512;

type Result<T> = std::result::Result<T, SocketError>;

#[derive(Diagnostic, Debug, thiserror::Error)]
pub enum SocketError {
    #[error("{path} is not set", path = niri_ipc::socket::SOCKET_PATH_ENV)]
    SocketPathNotSet,

    #[error("EOF while reading the stream")]
    EOF,

    #[error("event stream request is denied by niri")]
    EventStreamRequestDenied,

    #[error("niri error: {0}")]
    NiriError(String),

    #[error(transparent)]
    ParseError(#[from] serde_json::Error),

    #[error(transparent)]
    CodecError(#[from] CodecError),

    #[error(transparent)]
    Io(#[from] io::Error),
}

pub struct EventSocket {
    handler: EventHandler,
}

#[allow(unused)]
impl EventSocket {
    pub async fn connect() -> Result<Self> {
        let path = match env::var_os(SOCKET_PATH_ENV) {
            Some(path) => path,
            None => return Err(SocketError::SocketPathNotSet),
        };
        let mut stream = UnixStream::connect(path).await?;

        stream.write_all(b"\"EventStream\"\n").await?;
        stream.shutdown().await?;

        let mut buf = [0u8; 10]; // "Handled"\n -> 10 bytes
        stream.read_exact(&mut buf).await?;

        if &buf == b"\"Handled\"\n" {
            let handler = EventHandler::new(stream);
            Ok(Self { handler })
        } else {
            Err(SocketError::EventStreamRequestDenied)
        }
    }

    pub async fn read_event(&mut self) -> Result<Event> {
        let event = self.handler.next().await?;
        Ok(event)
    }
}

pub struct BidiSocket {
    handler: BidiHandler,
}

#[allow(unused)]
impl BidiSocket {
    pub async fn connect() -> Result<Self> {
        let path = match env::var_os(SOCKET_PATH_ENV) {
            Some(path) => path,
            None => return Err(SocketError::SocketPathNotSet),
        };
        let stream = UnixStream::connect(path).await?;
        let handler = BidiHandler::new(stream);
        Ok(Self { handler })
    }

    pub async fn send(&mut self, request: Request) -> Result<Response> {
        self.handler.send(request).await?;

        match self.handler.next().await? {
            Ok(response) => Ok(response),
            Err(message) => Err(SocketError::NiriError(message)),
        }
    }
}

trait Writer {
    async fn send(&mut self, request: Request) -> Result<()>;
}

trait Reader<T: serde::de::DeserializeOwned> {
    async fn next(&mut self) -> Result<T>;
}

struct EventHandler {
    stream: UnixStream,
    codec: Codec,
    buf: BytesMut,
}

impl EventHandler {
    fn new(stream: UnixStream) -> Self {
        Self {
            stream,
            codec: Codec::new(MAX_FRAME_LENGTH),
            buf: BytesMut::with_capacity(SOCKET_BUF_SIZE),
        }
    }
}

impl Reader<Event> for EventHandler {
    async fn next(&mut self) -> Result<Event> {
        loop {
            if let Some(frame) = self.codec.decode::<Event>(&mut self.buf)? {
                return Ok(frame);
            }

            if self.stream.read_buf(&mut self.buf).await? == 0 {
                return Err(SocketError::EOF);
            }
        }
    }
}

struct BidiHandler {
    stream: UnixStream,
    codec: Codec,
    buf_read: BytesMut,
    buf_write: BytesMut,
}

impl BidiHandler {
    fn new(stream: UnixStream) -> Self {
        Self {
            stream,
            codec: Codec::new(MAX_FRAME_LENGTH),
            buf_read: BytesMut::with_capacity(SOCKET_BUF_SIZE),
            buf_write: BytesMut::with_capacity(SOCKET_BUF_SIZE),
        }
    }
}

impl Reader<Reply> for BidiHandler {
    async fn next(&mut self) -> Result<Reply> {
        loop {
            if let Some(frame) = self.codec.decode::<Reply>(&mut self.buf_read)? {
                return Ok(frame);
            }

            if self.stream.read_buf(&mut self.buf_read).await? == 0 {
                return Err(SocketError::EOF);
            }
        }
    }
}

impl Writer for BidiHandler {
    async fn send(&mut self, request: Request) -> Result<()> {
        self.codec.encode(&request, &mut self.buf_write)?;
        self.stream.write_all(&self.buf_write).await?;
        self.buf_write.clear();
        Ok(())
    }
}

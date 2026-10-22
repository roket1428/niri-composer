use std::env;

use bytes::BytesMut;
use niri_ipc::{Event, Reply, Request, Response, socket::SOCKET_PATH_ENV};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tracing::info;

use crate::codec::Codec;
use crate::errors::{Context, SocketError};

const SOCKET_BUF_SIZE: usize = 2048;
const MAX_FRAME_LENGTH: usize = 512;

type Result<T> = std::result::Result<T, SocketError>;

#[derive(Debug)]
pub struct EventSocket {
    handler: EventHandler,
}

impl EventSocket {
    pub async fn connect() -> Result<Self> {
        let path = env::var_os(SOCKET_PATH_ENV).ok_or(SocketError::PathNotSet)?;
        info!("Connecting to (event): {:?}", &path);
        let mut stream = UnixStream::connect(path)
            .await
            .context("Failed to connect to niri (event socket)")?;

        stream
            .write_all(b"\"EventStream\"\n")
            .await
            .context("Failed to write bytes into event socket")?;

        stream
            .shutdown()
            .await
            .context("Failed to close the write half of the event socket")?;

        let mut buf = [0u8; 17]; // {"Ok":"Handled"}\n -> 17 bytes
        stream
            .read_exact(&mut buf)
            .await
            .context("Failed to read bytes into event buffer while requesting event stream")?;

        if &buf == b"{\"Ok\":\"Handled\"}\n" {
            Ok(Self {
                handler: EventHandler::new(stream),
            })
        } else {
            Err(SocketError::EventStreamDenied)
        }
    }

    pub async fn read_event(&mut self) -> Result<Event> {
        self.handler.next().await
    }
}

#[derive(Debug)]
pub struct BidiSocket {
    handler: BidiHandler,
}

#[allow(unused)]
impl BidiSocket {
    pub async fn connect() -> Result<Self> {
        let path = env::var_os(SOCKET_PATH_ENV).ok_or(SocketError::PathNotSet)?;
        info!("Connecting to (bidi): {:?}", &path);
        let stream = UnixStream::connect(path)
            .await
            .context("Failed to connect to bidi socket")?;

        Ok(Self {
            handler: BidiHandler::new(stream),
        })
    }

    pub async fn send(&mut self, request: Request) -> Result<Response> {
        self.handler.send(request).await?;
        self.handler
            .next()
            .await?
            .map_err(|msg| SocketError::NiriError { msg })
    }
}

trait Writer {
    async fn send(&mut self, request: Request) -> Result<()>;
}

trait Reader<T: serde::de::DeserializeOwned> {
    async fn next(&mut self) -> Result<T>;
}

#[derive(Debug)]
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

            let n = self
                .stream
                .read_buf(&mut self.buf)
                .await
                .context("Failed to read bytes into event buffer")?;

            if n == 0 {
                return Err(SocketError::EOF);
            }
        }
    }
}

#[derive(Debug)]
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

            let n = self
                .stream
                .read_buf(&mut self.buf_read)
                .await
                .context("Failed to read bytes into bidi buffer")?;

            if n == 0 {
                return Err(SocketError::EOF);
            }
        }
    }
}

impl Writer for BidiHandler {
    async fn send(&mut self, request: Request) -> Result<()> {
        self.codec.encode(&request, &mut self.buf_write)?;
        self.stream
            .write_all(&self.buf_write)
            .await
            .context("Failed to write bytes into bidi socket")?;

        self.buf_write.clear();
        Ok(())
    }
}

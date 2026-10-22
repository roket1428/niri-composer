use std::io::Write;
use std::os::unix::net::UnixStream;

use crate::errors::{ClientError, Context};

const CLIENT_SOCKET: &'static str = "/tmp/niri-composer.socket";

pub fn dispatch(mut command: String) -> Result<(), ClientError> {
    command.insert_str(0, "d://");

    #[rustfmt::skip]
    let mut stream = UnixStream::connect(CLIENT_SOCKET)
        .context("Couldn't connect to daemon, is it running?")?;

    stream
        .write_all(&command.as_bytes())
        .context("Couldn't write to daemon socket")?;

    Ok(())
}

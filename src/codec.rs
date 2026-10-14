use bytes::{Buf, BufMut, BytesMut};
use memchr::memchr;
use std::{cmp, io};

// TODO: maybe ditch thiserror
#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("max line length exceeded")]
    MaxLineLengthExceeded,

    #[error(transparent)]
    ParseError(#[from] serde_json::Error),

    #[error(transparent)]
    Io(#[from] io::Error),
}

type Result<T> = std::result::Result<T, CodecError>;

pub struct Codec {
    next_index: usize,
    max_length: usize,
    is_discarding: bool,
}

impl Codec {
    pub fn new(max_length: usize) -> Self {
        Self {
            max_length,
            next_index: 0,
            is_discarding: false,
        }
    }

    pub fn decode<T: serde::de::DeserializeOwned>(
        &mut self,
        buf: &mut BytesMut,
    ) -> Result<Option<T>> {
        loop {
            let read_to = cmp::min(self.max_length.saturating_add(1), buf.len());
            let newline_offset = memchr(b'\n', &buf[self.next_index..read_to]);
            match (self.is_discarding, newline_offset) {
                (true, Some(offset)) => {
                    // If we found a newline, discard up to that offset and
                    // then stop discarding. On the next iteration, we'll try
                    // to read a line normally.
                    buf.advance(offset + self.next_index + 1);
                    self.is_discarding = false;
                    self.next_index = 0;
                }
                (true, None) => {
                    // Otherwise, we didn't find a newline, so we'll discard
                    // everything we read. On the next iteration, we'll continue
                    // discarding up to max_len bytes unless we find a newline.
                    buf.advance(read_to);
                    self.next_index = 0;
                    if buf.is_empty() {
                        return Ok(None);
                    }
                }
                (false, Some(offset)) => {
                    // Found a line!
                    let newline_index = offset + self.next_index;
                    self.next_index = 0;
                    let response = serde_json::from_slice(&buf[..newline_index])?;
                    buf.advance(newline_index + 1);
                    return Ok(Some(response));
                }
                (false, None) if buf.len() > self.max_length => {
                    // Reached the maximum length without finding a
                    // newline, return an error and start discarding on the
                    // next call.
                    self.is_discarding = true;
                    return Err(CodecError::MaxLineLengthExceeded);
                }
                (false, None) => {
                    // We didn't find a line or reach the length limit, so the next
                    // call will resume searching at the current offset.
                    self.next_index = read_to;
                    return Ok(None);
                }
            }
        }
    }

    pub fn encode<T: serde::ser::Serialize>(
        &mut self,
        request: &T,
        buf: &mut BytesMut,
    ) -> Result<()> {
        let request = serde_json::to_string(request)?;
        buf.reserve(request.len() + 1);
        buf.put(request.as_bytes());
        buf.put_u8(b'\n');
        Ok(())
    }
}

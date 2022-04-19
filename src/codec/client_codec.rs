use bytes::{BytesMut, BufMut, Bytes};
use tokio_util::codec::{Decoder, Encoder};
use std::{io, str};

#[derive(Debug)]
pub enum ClientCodecError {
    Io(io::Error),
}

impl From<io::Error> for ClientCodecError {
    fn from(e: io::Error) -> ClientCodecError {
        ClientCodecError::Io(e)
    }
}

#[derive(Debug)]
enum BatDecoderStatus {
    Text,
    Esc,
}

#[derive(Debug)]
pub struct ClientCodec {
    // Stored index of the next index to examine for characters that might
    // change decoder status.
    next_index: usize,

    // Stored the current decoding status.
    decoder_status: BatDecoderStatus,
}

impl ClientCodec {
    pub fn new() -> ClientCodec {
        ClientCodec {
            next_index: 0,
            decoder_status: BatDecoderStatus::Text,
        }
    }
}

fn utf8(buf: &[u8]) -> Result<&str, io::Error> {
    str::from_utf8(buf)
        .map_err(|e| {
            log::error!("{}, {:?}", e, buf);
            io::Error::new(io::ErrorKind::InvalidData, "[client] Unable to decode input as UTF8")
        })
}

impl Decoder for ClientCodec {
    type Item = Bytes;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Bytes>, io::Error> {
        if !buf.is_empty() {
            let len = buf.len();
            Ok(Some(buf.split_to(len).freeze()))
        } else {
            Ok(None)
        }
    }
}

impl<T> Encoder<T> for ClientCodec
where
    T: Into<Bytes>,
{
    type Error = io::Error;

    fn encode(&mut self, item: T, buf: &mut BytesMut) -> Result<(), Self::Error> {
        let bytes = item.into();
        log::debug!("send to client: {:?}", bytes);
        buf.reserve(bytes.len());
        buf.put(bytes);
        Ok(())
    }
}

use bytes::{BufMut, BytesMut};
use tokio_io::codec::{Encoder, Decoder};
use std::io;

use super::super::protocol::*;

#[derive(Debug)]
pub enum SendFrame {
    Line(BytesMut),
    MonsterExp(String, String, String),
    Error
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct LinesCodec {
    next_index: usize,
}

impl LinesCodec {
    pub fn new() -> LinesCodec {
        LinesCodec { next_index: 0 }
    }
}

fn process_line(mut bytes: BytesMut) -> SendFrame {
    if bytes.len() > 15 && &bytes[..15] == b"::monster_exp::" {
        let mut name = bytes.split_off(15);

        if let Some(i) = name.iter().position(|&x| x == b':') {
            let mut area = name.split_off(i);
            if let Some(i) = area.iter().position(|&x| x == b':') {
                let exp = area.split_off(i);
                SendFrame::MonsterExp(latin1_to_string(&name), latin1_to_string(&area), latin1_to_string(&exp))
            } else {
                SendFrame::Error
            }
        } else {
            SendFrame::Error
        }

    } else {
        SendFrame::Line(bytes)
    }
}

impl Decoder for LinesCodec {
    type Item = SendFrame;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<SendFrame>, io::Error> {
        println!("{:?}", buf);
        if let Some(newline_offset) =
            buf[self.next_index..].iter().position(|b| *b == b'\n')
        {
            let newline_index = newline_offset + self.next_index;
            let line = buf.split_to(newline_index + 1);
            self.next_index = 0;
            Ok(Some(process_line(line)))
        } else {
            self.next_index = buf.len();
            Ok(None)
        }
    }

    fn decode_eof(&mut self, buf: &mut BytesMut) -> Result<Option<SendFrame>, io::Error> {
        Ok(match self.decode(buf)? {
            Some(frame) => Some(frame),
            None => {
                // No terminating newline - return remaining data, if any
                if buf.is_empty() || buf == &b"\r"[..] {
                    None
                } else {
                    let line = buf.take();
                    self.next_index = 0;
                    Some(process_line(line))
                }
            }
        })
    }
}

impl Encoder for LinesCodec {
    type Item = String;
    type Error = io::Error;

    fn encode(&mut self, line: String, buf: &mut BytesMut) -> Result<(), io::Error> {
        buf.reserve(line.len() + 1);
        buf.put(line);
        buf.put_u8(b'\n');
        Ok(())
    }
}

use bytes::{BufMut, BytesMut};
use tokio_io::codec::{Encoder, Decoder};
use std::io;
use regex::Regex;

use super::super::protocol::*;

#[derive(Debug)]
pub enum SendFrame {
    Line(BytesMut),
    MonsterExp(String, String, i32),
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
    if bytes.len() > 2 && &bytes[..2] == b";;" {
        let re = Regex::new(r";;").unwrap();
        let len = bytes.len() - 2;
        let s = latin1_to_string(&bytes.split_to(len));
        let fields: Vec<&str> = re.split(s.as_str()).collect();

        if fields[1] == "monster:exp" && fields.len() == 5 {
            SendFrame::MonsterExp(
                fields[2].to_string(),
                fields[3].to_string(),
                fields[4].to_string().parse::<i32>().unwrap()
            )
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
        if let Some(newline_offset) =
            buf[self.next_index..].iter().position(|b| *b == b'\n') {
            let newline_index = newline_offset + self.next_index;
            let line = buf.split_to(newline_index + 1);
            self.next_index = 0;
            Ok(Some(process_line(line)))
        } else if buf.len() > 0 {
            let line = buf.take();
            self.next_index = 0;
            Ok(Some(process_line(line)))
        } else {
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

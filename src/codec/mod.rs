use bytes::{BufMut, BytesMut};
use std::io;
use tokio_util::codec::Decoder;

use self::control_code::ControlCode;

mod control_code;

#[derive(Debug, PartialEq)]
pub enum BatMudFrame {
    Text(Vec<u8>),
    Code(control_code::ControlCode),
    Continue,
}

pub struct BatMudCodec {
    state: BatMudCodecState,
    next_index: usize,
    current_code_stack: Vec<(u8, u8)>,
}

impl BatMudCodec {
    pub fn new() -> Self {
        Self {
            state: BatMudCodecState::Text,
            next_index: 0,
            current_code_stack: Vec::new(),
        }
    }

    fn decode_text(&mut self, buf: &mut BytesMut) -> io::Result<Option<BatMudFrame>> {
        let break_offset = buf[self.next_index..buf.len()]
            .iter()
            .position(|b| *b == b'\n' || *b == b'\x1b');

        match break_offset {
            Some(offset) if buf[self.next_index + offset] == b'\x1b' => {
                self.state = BatMudCodecState::Esc;
                let break_index = self.next_index + offset;

                if break_index > 0 && self.current_code_stack.is_empty() {
                    let mut line = buf.split_to(break_index);
                    line.put_u8(b'\n');
                    self.next_index = 1;
                    Ok(Some(BatMudFrame::Text(line.to_vec())))
                } else {
                    self.next_index += offset + 1;
                    Ok(Some(BatMudFrame::Continue))
                }
            }

            Some(offset) if self.current_code_stack.is_empty() => {
                let newline_index = offset + self.next_index;
                let line = buf.split_to(newline_index + 1);
                self.next_index = 0;
                Ok(Some(BatMudFrame::Text(line.to_vec())))
            }

            Some(offset) => {
                self.next_index += offset + 1;
                Ok(Some(BatMudFrame::Continue))
            }

            None => {
                self.next_index = buf.len();
                Ok(None)
            }
        }
    }

    fn decode_esc(&mut self, buf: &mut BytesMut) -> io::Result<Option<BatMudFrame>> {
        // we have seen an escape character, so "<nn", ">nn" or "|" are expected,
        if buf.len() > self.next_index + 2 {
            match buf[self.next_index] {
                b'<' => {
                    let opening = (buf[self.next_index + 1], buf[self.next_index + 2]);
                    self.current_code_stack.push(opening);
                    self.next_index += 3;
                    self.state = BatMudCodecState::Text;
                    Ok(Some(BatMudFrame::Continue))
                }

                b'>' => {
                    let opening = self.current_code_stack.pop();
                    let closing = (buf[self.next_index + 1], buf[self.next_index + 2]);

                    if opening.is_none() || opening.unwrap() != closing {
                        let discarded = buf.split_to(self.next_index + 3);
                        tracing::error!(
                            "Closing code {:?} is not the current opening code {:?}. Code stack (if any) will be discarded.",
                            closing,
                            opening,
                        );
                        tracing::error!("Discarded bytes: {:?}", discarded);

                        self.current_code_stack.clear();
                        self.next_index = 0;
                        self.state = BatMudCodecState::Text;

                        Ok(Some(BatMudFrame::Continue))
                    } else if self.current_code_stack.is_empty() {
                        let bytes = buf.split_to(self.next_index + 3);
                        let code = ControlCode::from(bytes)?;
                        self.state = BatMudCodecState::Text;
                        self.next_index = 0;
                        Ok(Some(BatMudFrame::Code(code)))
                    } else {
                        self.state = BatMudCodecState::Text;
                        self.next_index += 3;
                        Ok(Some(BatMudFrame::Continue))
                    }
                }

                _ => {
                    self.state = BatMudCodecState::Text;
                    self.next_index += 1;
                    Ok(Some(BatMudFrame::Continue))
                }
            }
        } else {
            Ok(None)
        }
    }
}

impl Decoder for BatMudCodec {
    type Item = BatMudFrame;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> io::Result<Option<Self::Item>> {
        match self.state {
            BatMudCodecState::Text => self.decode_text(src),
            BatMudCodecState::Esc => self.decode_esc(src),
            _ => todo!(),
        }
    }
}

enum BatMudCodecState {
    Text,
    Esc,
    IAC(u8),
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! decode_buf {
        ($buf:expr) => {{
            let mut buf = BytesMut::from($buf);
            let mut codec = BatMudCodec::new();
            let mut frames: Vec<BatMudFrame> = vec![];
            while let Ok(Some(frame)) = codec.decode(&mut buf) {
                if frame != BatMudFrame::Continue {
                    frames.push(frame);
                }
            }

            assert!(buf.is_empty());

            frames
        }};
    }

    #[test]
    fn decode_text() {
        let frames = decode_buf!(&b"Hello, world!\n"[..]);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], BatMudFrame::Text(b"Hello, world!\n".to_vec()));
    }

    #[test]
    fn decode_control_code_after_text() {
        let frames = decode_buf!(&b"Hello, world!\x1b<00foo\x1b>00"[..]);
        assert_eq!(frames.len(), 2);

        assert_eq!(frames[0], BatMudFrame::Text(b"Hello, world!\n".to_vec()));
        assert_eq!(
            frames[1],
            BatMudFrame::Code(ControlCode::from(b"\x1b<00foo\x1b>00"[..].into()).unwrap())
        );
    }

    #[test]
    fn decode_control_code_before_text() {
        let frames = decode_buf!(&b"\x1b<00foo\x1b>00Hello, world!\n"[..]);
        assert_eq!(frames.len(), 2);

        assert_eq!(
            frames[0],
            BatMudFrame::Code(ControlCode::from(b"\x1b<00foo\x1b>00"[..].into()).unwrap())
        );
        assert_eq!(frames[1], BatMudFrame::Text(b"Hello, world!\n".to_vec()));
    }

    #[test]
    fn decode_control_code_around_text() {
        let frames = decode_buf!(&b"\x1b<00foo\x1b>00Hello, world!\x1b<00bar\x1b>00"[..]);
        assert_eq!(frames.len(), 3);

        assert_eq!(
            frames[0],
            BatMudFrame::Code(ControlCode::from(b"\x1b<00foo\x1b>00"[..].into()).unwrap())
        );
        assert_eq!(frames[1], BatMudFrame::Text(b"Hello, world!\n".to_vec()));
        assert_eq!(
            frames[2],
            BatMudFrame::Code(ControlCode::from(b"\x1b<00bar\x1b>00"[..].into()).unwrap())
        );
    }

    #[test]
    fn decode_control_codes_with_attributes() {
        let frames = decode_buf!(&b"\x1b<20FF0000\x1b|This is a test\x1b>20"[..]);
        assert_eq!(frames.len(), 1);

        assert_eq!(
            frames[0],
            BatMudFrame::Code(
                ControlCode::from(b"\x1b<20FF0000\x1b|This is a test\x1b>20"[..].into()).unwrap()
            )
        );
    }

    #[test]
    fn decode_stacked_control_codes() {
        let frames = decode_buf!(&b"\x1b<10map\x1b|\x1b<11\x1b>11\x1b>10"[..]);
        assert_eq!(frames.len(), 1);

        assert_eq!(
            frames[0],
            BatMudFrame::Code(
                ControlCode::from(b"\x1b<10map\x1b|\x1b<11\x1b>11\x1b>10"[..].into()).unwrap()
            )
        );
    }
}

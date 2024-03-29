use bytes::BytesMut;
use std::{
    fmt::{self, Display, Formatter},
    io,
};
use tokio_util::codec::Decoder;

use self::control_code::ControlCode;

pub(crate) mod control_code;

const IAC: u8 = 0xff;

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
            .position(|b| *b == b'\n' || *b == b'\x1b' || *b == IAC);

        match break_offset {
            Some(offset) if buf[self.next_index + offset] == IAC => {
                if self.next_index + offset + 1 < buf.len() {
                    let line = buf.split_to(self.next_index + offset + 2);
                    self.next_index = 0;
                    Ok(Some(BatMudFrame::Text(line.to_vec())))
                } else {
                    Ok(None)
                }
            }

            Some(offset) if buf[self.next_index + offset] == b'\x1b' => {
                self.state = BatMudCodecState::Esc;
                self.next_index += offset + 1;
                Ok(Some(BatMudFrame::Continue))
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
                    // we are at the byte after \x1b, if next_index > 1, there are  some
                    // bytes before \x1b that are not part of this opening control code,
                    // which might need to be split into a text frame
                    if self.next_index > 1 && self.current_code_stack.is_empty() {
                        let line = buf.split_to(self.next_index - 1);
                        self.next_index = 1;
                        Ok(Some(BatMudFrame::Text(line.to_vec())))
                    } else {
                        let opening = (buf[self.next_index + 1], buf[self.next_index + 2]);
                        self.current_code_stack.push(opening);
                        self.next_index += 3;
                        self.state = BatMudCodecState::Text;
                        Ok(Some(BatMudFrame::Continue))
                    }
                }

                b'>' => {
                    let opening = self.current_code_stack.pop();
                    let closing = (buf[self.next_index + 1], buf[self.next_index + 2]);

                    if opening.is_none() || opening.unwrap() != closing {
                        // after splitting, `buf` will contain all bytes right before the closing tag
                        let mut rest = buf.split_off(self.next_index - 1);

                        // discard 4 bytes to discard the closing tag
                        let discarded = rest.split_to(4);

                        // concatenate the rest of the buffer back to `buf`
                        buf.extend(rest);

                        tracing::debug!(
                            "Closing code {}{} is not the current opening code {}. This closing tag will be discarded.",
                            closing.0 as char, closing.1 as char,
                            opening.map(|(a, b)| format!("{}{}", a as char, b as char)).unwrap_or_else(|| "None".to_string()),
                        );
                        tracing::debug!("Discarded bytes: {:?}", discarded);

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
        tracing::debug!(
            "decoding {} [{}/{}]: {:?}",
            self.state,
            self.next_index,
            src.len(),
            src
        );

        if src.is_empty() {
            return Ok(None);
        }

        match self.state {
            BatMudCodecState::Text => self.decode_text(src),
            BatMudCodecState::Esc => self.decode_esc(src),
        }
    }
}

enum BatMudCodecState {
    Text,
    Esc,
}

impl Display for BatMudCodecState {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            BatMudCodecState::Text => write!(f, "[TXT]"),
            BatMudCodecState::Esc => write!(f, "[ESC]"),
        }
    }
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

        assert_eq!(frames[0], BatMudFrame::Text(b"Hello, world!".to_vec()));
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
        assert_eq!(frames[1], BatMudFrame::Text(b"Hello, world!".to_vec()));
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

    #[test]
    fn decode_control_codes_with_redundant_closing_tag() {
        let frames = decode_buf!(&b"\x1b<10map\x1b|\x1b<200000FF\x1b|foo\x1b>20\x1b>20\x1b>10"[..]);
        assert_eq!(frames.len(), 1);
        assert_eq!(
            frames[0],
            BatMudFrame::Code(
                ControlCode::from(b"\x1b<10map\x1b|\x1b<200000FF\x1b|foo\x1b>20\x1b>10"[..].into())
                    .unwrap()
            )
        );
    }

    #[test]
    fn decode_control_codes_with_newline_in_value() {
        let frames = decode_buf!(&b"\x1b<10map\x1b|\x1b<200000FF\x1b|foo\nbar\x1b>20\x1b>10"[..]);
        assert_eq!(frames.len(), 1);
        assert_eq!(
            frames[0],
            BatMudFrame::Code(
                ControlCode::from(
                    b"\x1b<10map\x1b|\x1b<200000FF\x1b|foo\nbar\x1b>20\x1b>10"[..].into()
                )
                .unwrap()
            )
        );
    }

    #[test]
    fn decode_text_with_color_escape_code() {
        let frames = decode_buf!(&b"The beginning\x1b[0m.\r\n"[..]);
        assert_eq!(frames.len(), 1);
        assert_eq!(
            frames[0],
            BatMudFrame::Text(b"The beginning\x1b[0m.\r\n".to_vec())
        );
    }

    #[test]
    fn decode_channel_message() {
        let frames = decode_buf!(&b"\x1b<10chan_newbie\x1b|\x1b[1;33mNyriori the Helper [newbie]: oh, well fair\x1b[0m\r\n\x1b>10"[..]);
        assert_eq!(frames.len(), 1);
        assert_eq!(
            frames[0],
            BatMudFrame::Code(
                ControlCode::from(
                    b"\x1b<10chan_newbie\x1b|\x1b[1;33mNyriori the Helper [newbie]: oh, well fair\x1b[0m\r\n\x1b>10"[..].into()
                ).unwrap()
            )
        );
    }
}

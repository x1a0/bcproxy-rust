use std::io;

use bytes::{Bytes, BufMut, BytesMut};
use tokio_io::codec::{Encoder, Decoder};

use super::protocol::{ControlCode, BatMapper, Monster};

#[derive(Debug)]
pub enum BatFrame {
    Bytes(BytesMut),
    Code(Box<ControlCode>),
    BatMapper(Box<BatMapper>),
    Monster(Box<Monster>),
    Nothing,
}

#[derive(Debug)]
enum State {
    Text,
    Esc,
    Open(Option<u8>),
    Close(Option<u8>),
    IAC(Option<u8>),
}

#[derive(Debug)]
pub struct BatCodec {
    state: State,
    next_index: usize,
    code: Option<Box<ControlCode>>,
    bat_mapper: Option<Box<BatMapper>>,
    at_line_start: bool,
    prompt_buf: BytesMut,
}

impl BatCodec {
    pub fn new() -> BatCodec {
        BatCodec {
            state: State::Text,
            next_index: 0,
            code: None,
            bat_mapper: None,
            at_line_start: true,
            prompt_buf: BytesMut::with_capacity(256),
        }
    }

    fn process(&mut self, bytes: BytesMut) -> BatFrame {
        let end_with_line_break = bytes.last().map(|&b| b == b'\n');

        let frame = match self.code {
            Some(ref mut code) => {
                code.body.reserve(bytes.len());
                code.body.put(bytes);
                BatFrame::Nothing
            },

            None if bytes.len() > 0 => {
                // plain bytes output
                // try to match mob names here
                // color code used here MUST match ansi settings in BatMUD
                if bytes.starts_with(b"\x1b[31m") {
                    let monster = Monster::new(
                        &bytes,
                        self.bat_mapper.clone().and_then(|x| x.area),
                        self.bat_mapper.clone().and_then(|x| x.id),
                        true
                    );
                    BatFrame::Monster(Box::new(monster))
                } else if bytes.starts_with(b"\x1b[32m") {
                    let monster = Monster::new(
                        &bytes,
                        self.bat_mapper.clone().and_then(|x| x.area),
                        self.bat_mapper.clone().and_then(|x| x.id),
                        false
                    );
                    BatFrame::Monster(Box::new(monster))
                } else {
                    BatFrame::Bytes(bytes)
                }
            },

            None => {
                BatFrame::Nothing
            }
        };

        if end_with_line_break.is_some() {
            self.at_line_start = end_with_line_break.unwrap();
        }

        frame
    }

    fn on_code_open(&mut self, (c1, c2): (u8, u8)) {
        let new_code = ControlCode::new((c1, c2), self.code.as_ref().map(|x| x.clone()));
        self.code = Some(Box::new(new_code));
    }

    fn on_code_close(&mut self, (c1, c2): (u8, u8)) -> BatFrame {
        match self.code.clone() {
            Some(ref mut code) if code.id == (c1, c2) => {
                match code.parent.clone() {
                    Some(parent) => {
                        let child_bytes = code.to_bytes();
                        self.code = Some(parent);
                        let frame = self.process(child_bytes);
                        frame
                    },

                    None if (c1, c2) == (b'1', b'0') && &code.attr[..] == b"spec_prompt" => {
                        self.prompt_buf.clear();
                        self.prompt_buf.extend(code.body.clone());
                        self.code = None;
                        BatFrame::Nothing
                    },

                    None if (c1, c2) == (b'9', b'9') => {
                        let bytes = code.body.split_off(12);
                        let bat_mapper = Box::new(BatMapper::new(bytes, self.bat_mapper.clone()));

                        if bat_mapper.id.is_none() {
                            self.bat_mapper = None;
                        } else {
                            self.bat_mapper = Some(bat_mapper.clone());
                        }

                        self.code = None;
                        BatFrame::BatMapper(bat_mapper)
                    },

                    None => {
                        // top level code closed
                        let frame = BatFrame::Code(code.clone());
                        self.at_line_start = code.end_with_line_break();
                        self.code = None;
                        frame
                    }
                }
            },

            _ => {
                debug!("discard unmatching close code {}{} and current code {:?}", c1, c2, self.code);
                BatFrame::Nothing
            },
        }
    }

    fn transition_to(&mut self, state: State) {
        let id = self.code.clone().map_or(('-', '-'), |c| (c.id.0 as char, c.id.1 as char));
        debug!("State transition from {:?} to {:?}. Current code: {}{}", self.state, state, id.0, id.1);
        self.state = state;
    }
}

impl Decoder for BatCodec {
    type Item = BatFrame;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<BatFrame>, io::Error> {
        if buf.len() <= self.next_index {
            Ok(None)
        } else {
            match self.state {
                State::Text => {
                    if let Some(offset) = buf[self.next_index..].iter().position(|&b| b == b'\x1b' || b == b'\xff') {
                        let index = self.next_index + offset;

                        if buf[index] == b'\xff' {
                            let mut bytes = buf.split_to(index + 1);
                            self.next_index = 0;
                            let len = bytes.len();
                            bytes.split_off(len - 1);
                            let frame = self.process(bytes);
                            self.transition_to(State::IAC(None));
                            Ok(Some(frame))
                        } else {
                            self.next_index = index + 1;
                            self.transition_to(State::Esc);
                            Ok(Some(BatFrame::Nothing))
                        }
                    } else {
                        self.next_index = buf.len();
                        Ok(Some(BatFrame::Nothing))
                    }
                },

                State::Esc => {
                    match buf[self.next_index] {
                        b'<' => {
                            self.next_index += 1;
                            self.transition_to(State::Open(None));
                            Ok(Some(BatFrame::Nothing))
                        },

                        b'>' => {
                            self.next_index += 1;
                            self.transition_to(State::Close(None));
                            Ok(Some(BatFrame::Nothing))
                        },

                        b'|' => {
                            if let Some(ref mut code) = self.code {
                                // confirm received an attribute
                                let mut bytes = buf.split_to(self.next_index + 1);
                                self.next_index = 0;
                                let len = bytes.len();
                                bytes.truncate(len - 2);

                                if !code.body.is_empty() {
                                    let body = code.body.clone();
                                    code.attr.extend(body);
                                    code.body.clear();
                                }

                                code.attr.extend(bytes);
                            } else {
                                self.next_index += 1;
                            }

                            self.transition_to(State::Text);
                            Ok(Some(BatFrame::Nothing))
                        },

                        _ => {
                            self.next_index += 1;
                            self.transition_to(State::Text);
                            Ok(Some(BatFrame::Nothing))
                        },
                    }
                },

                State::Open(None) => {
                    match buf[self.next_index] {
                        c1 @ b'0' ..= b'9' => {
                            self.next_index += 1;
                            self.transition_to(State::Open(Some(c1)));
                            Ok(Some(BatFrame::Nothing))
                        },

                        _c1 => {
                            self.next_index += 1;
                            self.transition_to(State::Text);
                            Ok(Some(BatFrame::Nothing))
                        },
                    }
                },

                State::Open(Some(c1)) => {
                    match buf[self.next_index] {
                        c2 @ b'0' ..= b'9' => {
                            // confirm receiving a new cdoe
                            let mut bytes = buf.split_to(self.next_index + 1);
                            self.next_index = 0;

                            // process all bytes before ESC<NN
                            let len = bytes.len();
                            bytes.truncate(len - 4);
                            let frame = self.process(bytes);

                            self.on_code_open((c1, c2));
                            self.transition_to(State::Text);
                            Ok(Some(frame))
                        },

                        _c2 => {
                            self.next_index += 1;
                            self.transition_to(State::Text);
                            Ok(Some(BatFrame::Nothing))
                        },
                    }
                },

                State::Close(None) => {
                    match buf[self.next_index] {
                        c1 @ b'0' ..= b'9' => {
                            self.next_index += 1;
                            self.transition_to(State::Close(Some(c1)));
                            Ok(Some(BatFrame::Nothing))
                        },

                        _c1 => {
                            self.next_index += 1;
                            self.transition_to(State::Text);
                            Ok(Some(BatFrame::Nothing))
                        },
                    }
                },

                State::Close(Some(c1)) => {
                    match buf[self.next_index] {
                        c2 @ b'0' ..= b'9' => {
                            // confirm received a complete code
                            let mut bytes = buf.split_to(self.next_index + 1);
                            self.next_index = 0;

                            let len = bytes.len();
                            bytes.truncate(len - 4);
                            self.process(bytes);
                            let frame = self.on_code_close((c1, c2));
                            self.transition_to(State::Text);
                            Ok(Some(frame))
                        },

                        _c2 => {
                            self.next_index += 1;
                            self.transition_to(State::Text);
                            Ok(Some(BatFrame::Nothing))
                        },
                    }
                },

                State::IAC(None) => {
                    match buf[self.next_index] {
                        b'\xff' => {
                            // data byte 255
                            self.next_index += 1;
                            self.transition_to(State::Text);
                            Ok(Some(BatFrame::Nothing))
                        },

                        b'\xf9' if !self.prompt_buf.is_empty() => {
                            buf.advance(1);
                            self.next_index = 0;
                            self.prompt_buf.extend(&[b'\xff', b'\xf9'][..]);
                            let bytes = self.prompt_buf.take();
                            let frame = self.process(bytes);
                            self.transition_to(State::Text);
                            Ok(Some(frame))
                        }

                        c @ b'\xfb' ..= b'\xfe' => {
                            buf.advance(1);
                            self.transition_to(State::IAC(Some(c)));
                            Ok(Some(BatFrame::Nothing))
                        },

                        c => {
                            buf.advance(1);
                            self.next_index = 0;
                            let bytes = BytesMut::from(&[b'\xff', c][..]);
                            let frame = self.process(bytes);
                            self.transition_to(State::Text);
                            Ok(Some(frame))
                        },
                    }
                },

                State::IAC(Some(c)) => {
                    let x = buf[self.next_index];
                    buf.advance(1);
                    let bytes = BytesMut::from(&[b'\xff', c, x][..]);
                    let frame = self.process(bytes);
                    self.transition_to(State::Text);
                    Ok(Some(frame))
                },
            }
        }
    }
}

impl Encoder for BatCodec {
    type Item = Bytes;
    type Error = io::Error;

    fn encode(&mut self, data: Bytes, buf: &mut BytesMut) -> Result<(), io::Error> {
        buf.reserve(data.len());
        buf.put(data);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use env_logger;

    macro_rules! next_frame {
        ($codec:expr, $buf:expr) => {
            {
                let mut frame: BatFrame = BatFrame::Nothing;

                while let Some(f) = $codec.decode(&mut $buf).unwrap() {
                    match f {
                        BatFrame::Nothing => continue,
                        _ => {
                            frame = f;
                            break;
                        }
                    }
                }

                frame
            }
        }
    }

    #[test]
    fn code_stack() {
        let _ = env_logger::try_init();

        let mut buf = BytesMut::from(&b"\x1b<20FFFFFF\x1b|\x1b<210000FF\x1b|Test output, white on blue\x1b>21\x1b>20"[..]);
        let mut codec = BatCodec::new();

        assert!(match next_frame!(codec, buf) {
            BatFrame::Code(code) => {
                assert_eq!(code.id, (b'2', b'0'));
                assert_eq!(code.attr, &b"FFFFFF"[..]);
                assert_eq!(code.body, &b"\x1b[48;5;12mTest output, white on blue\x1b[0m"[..]);
                true
            },

            _ => false
        });

        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn code_in_plain_text() {
        let _ = env_logger::try_init();

        let mut buf = BytesMut::from(&b"foo\n\x1b<20FFFFFF\x1b|white text\n\x1b>20bar"[..]);
        let mut codec = BatCodec::new();

        assert!(codec.at_line_start);
        assert!(match next_frame!(codec, buf) {
            BatFrame::Bytes(ref bytes) => {
                assert_eq!(&bytes[..], b"foo\n");
                true
            },

            _ => false,
        });

        assert!(codec.at_line_start);
        assert!(match next_frame!(codec, buf) {
            BatFrame::Code(ref code) => {
                assert_eq!(code.id, (b'2', b'0'));
                assert_eq!(&code.attr[..], b"FFFFFF");
                assert_eq!(&code.body[..], b"white text\n");
                true
            },

            _ => false
        });

        assert!(codec.at_line_start);
        assert!(match next_frame!(codec, buf) {
            BatFrame::Bytes(ref bytes) => {
                assert_eq!(&bytes[..], b"bar");
                true
            },

            _ => false,
        });

        assert_eq!(buf.len(), 0);
    }
}

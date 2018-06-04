use std::io;

use bytes::{Bytes, BufMut, BytesMut};
use tokio_io::codec::{Encoder, Decoder};

use super::protocol::{ControlCode, BatMapper};

#[derive(Debug)]
pub enum BatFrame {
    Bytes(BytesMut),
    Code(Box<ControlCode>),
    BatMapper(BatMapper),
    Nothing,
}

#[derive(Debug)]
enum State {
    Text,
    Esc,
    Open(Option<u8>),
    Close(Option<u8>),
}

#[derive(Debug)]
pub struct BatCodec {
    state: State,
    next_index: usize,
    code: Option<Box<ControlCode>>,
}

impl BatCodec {
    pub fn new() -> BatCodec {
        BatCodec {
            state: State::Text,
            next_index: 0,
            code: None,
        }
    }

    fn process(&mut self, bytes: BytesMut) -> BatFrame {
        match self.code {
            Some(ref mut code) => {
                code.body.reserve(bytes.len());
                code.body.put(bytes);
                BatFrame::Nothing
            },

            None if bytes.len() > 0 => {
                BatFrame::Bytes(bytes)
            },

            None => {
                BatFrame::Nothing
            }
        }
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

                    None if (c1, c2) == (b'9', b'9') => {
                        let bat_mapper = BatMapper::new(code.body.split_off(12));
                        BatFrame::BatMapper(bat_mapper)
                    },

                    _ => {
                        let frame = BatFrame::Code(code.clone());
                        self.code = None;
                        frame
                    }
                }
            },

            _ => {
                warn!("discard unmatching close code {}{} and current code {:?}", c1, c2, self.code);
                self.code = None;
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
        if buf.is_empty() {
            Ok(None)
        } else {
            match self.state {
                State::Text => {
                    if let Some(offset) = buf.iter().position(|&b| b == b'\x1b') {
                        let mut bytes = buf.split_to(offset + 1);
                        let frame = self.process(bytes.split_to(offset));
                        self.transition_to(State::Esc);
                        Ok(Some(frame))
                    } else {
                        let frame = self.process(buf.take());
                        Ok(Some(frame))
                    }
                },

                State::Esc => {
                    match buf[0] {
                        b'<' => {
                            buf.advance(1);
                            self.transition_to(State::Open(None));
                            Ok(Some(BatFrame::Nothing))
                        },

                        b'>' => {
                            buf.advance(1);
                            self.transition_to(State::Close(None));
                            Ok(Some(BatFrame::Nothing))
                        },

                        b'|' => {
                            buf.advance(1);
                            self.transition_to(State::Text);

                            match self.code {
                                Some(ref mut code) => {
                                    code.attr = code.body.clone();
                                    code.body.clear();
                                    Ok(Some(BatFrame::Nothing))
                                },

                                None => {
                                    let frame = self.process(BytesMut::from(&[b'\x1b', b'|'][..]));
                                    Ok(Some(frame))
                                },
                            }
                        },

                        _ => {
                            let frame = self.process(BytesMut::from(&[b'\x1b'][..]));
                            self.transition_to(State::Text);
                            Ok(Some(frame))
                        },
                    }
                },

                State::Open(None) => {
                    match buf[0] {
                        c1 @ b'0' ..= b'9' => {
                            buf.advance(1);
                            self.transition_to(State::Open(Some(c1)));
                            Ok(Some(BatFrame::Nothing))
                        },

                        c1 => {
                            let frame = self.process(BytesMut::from(&[b'\x1b', b'<', c1][..]));
                            buf.advance(1);
                            self.transition_to(State::Text);
                            Ok(Some(frame))
                        },
                    }
                },

                State::Open(Some(c1)) => {
                    match buf[0] {
                        c2 @ b'0' ..= b'9' => {
                            self.on_code_open((c1, c2));
                            buf.advance(1);
                            self.transition_to(State::Text);
                            Ok(Some(BatFrame::Nothing))
                        },

                        c2 => {
                            let frame = self.process(BytesMut::from(&[b'\x1b', b'<', c1, c2][..]));
                            buf.advance(1);
                            self.transition_to(State::Text);
                            Ok(Some(frame))
                        },
                    }
                },

                State::Close(None) => {
                    match buf[0] {
                        c1 @ b'0' ..= b'9' => {
                            buf.advance(1);
                            self.transition_to(State::Close(Some(c1)));
                            Ok(Some(BatFrame::Nothing))
                        },

                        c1 => {
                            let frame = self.process(BytesMut::from(&[b'\x1b', b'>', c1][..]));
                            buf.advance(1);
                            self.transition_to(State::Text);
                            Ok(Some(frame))
                        },
                    }
                },

                State::Close(Some(c1)) => {
                    match buf[0] {
                        c2 @ b'0' ..= b'9' => {
                            let frame = self.on_code_close((c1, c2));
                            buf.advance(1);
                            self.transition_to(State::Text);
                            Ok(Some(frame))
                        },

                        c2 => {
                            let frame = self.process(BytesMut::from(&[b'\x1b', b'>', c1, c2][..]));
                            buf.advance(1);
                            self.transition_to(State::Text);
                            Ok(Some(frame))
                        },
                    }
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

    #[test]
    fn code_stack() {
        let _ = env_logger::try_init();

        let mut input = BytesMut::from(&b"\x1b<20FFFFFF\x1b|\x1b<210000FF\x1b|Test output, white on blue\x1b>21\x1b>20"[..]);
        let mut codec = BatCodec::new();

        let mut frame: BatFrame = BatFrame::Nothing;
        while let Ok(Some(f)) = codec.decode(&mut input) {
            frame = f;
        }

        assert!(match frame {
            BatFrame::Code(code) => {
                assert_eq!(code.id, (b'2', b'0'));
                assert_eq!(code.attr, &b"FFFFFF"[..]);
                assert_eq!(code.body, &b"\x1b[48;5;12mTest output, white on blue\x1b[0m"[..]);
                true
            },

            _ => false
        });
    }

    #[test]
    fn code_in_plain_text() {
        let _ = env_logger::try_init();

        let mut input = BytesMut::from(&b"foo\n\x1b<20FFFFFF\x1b|white text\x1b>20bar"[..]);
        let mut codec = BatCodec::new();

        let mut frame: BatFrame = BatFrame::Nothing;
        while let Ok(Some(f)) = codec.decode(&mut input) {
            match f {
                BatFrame::Bytes(_) => {
                    frame = f;
                    break
                },

                _ => continue,
            }
        }

        assert!(match frame {
            BatFrame::Bytes(ref bytes) => {
                assert_eq!(&bytes[..], b"foo\n");
                true
            },

            _ => false
        });

        while let Ok(Some(f)) = codec.decode(&mut input) {
            match f {
                BatFrame::Code(_) => {
                    frame = f;
                    break
                },

                _ => continue,
            }
        }

        assert!(match frame {
            BatFrame::Code(ref code) => {
                assert_eq!(code.id, (b'2', b'0'));
                assert_eq!(&code.attr[..], b"FFFFFF");
                assert_eq!(&code.body[..], b"white text");
                true
            },

            _ => false
        });

        while let Ok(Some(f)) = codec.decode(&mut input) {
            match f {
                BatFrame::Bytes(_) => {
                    frame = f;
                    break
                },

                _ => continue,
            }
        }

        assert!(match frame {
            BatFrame::Bytes(ref bytes) => {
                assert_eq!(&bytes[..], b"bar");
                true
            },

            _ => false
        });
    }
}

use std::{io, fmt};

use bytes::{Bytes, BufMut, BytesMut};
use tokio_io::codec::{Encoder, Decoder};

use super::protocol::ControlCode;

#[derive(Debug)]
pub struct BatCodec {
    state: State,
    next_index: usize,
    code: Option<Box<ControlCode>>,
}

#[derive(Debug)]
enum State {
    Text,
    Esc,
    Open(Option<u8>, Option<u8>),
    Close(Option<u8>, Option<u8>),
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            State::Text =>
                write!(f, "Text"),
            State::Esc =>
                write!(f, "Esc"),
            State::Open(c1, c2)  =>
                write!(f, "Open ({}{})", c1.map_or('-', |c| c as char), c2.map_or('-', |c| c as char)),
            State::Close(c1, c2) =>
                write!(f, "Close ({}{})", c1.map_or('-', |c| c as char), c2.map_or('-', |c| c as char)),
        }
    }
}

#[derive(Debug)]
pub enum BatFrame {
    Bytes(BytesMut),
    Code(Box<ControlCode>),
    Nothing,
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

    fn transition_to(&mut self, state: State) {
        let id = self.code.clone().map_or(('-', '-'), |c| (c.id.0 as char, c.id.1 as char));
        debug!("State transition from {} to {}. Current code: {}{}", self.state, state, id.0, id.1);
        self.state = state;
    }
}

const BYTE_ESC: u8 = b'\x1b';
const BYTE_OPEN: u8 = b'<';
const BYTE_CLOSE: u8 = b'>';
const BYTE_PIPE: u8 = b'|';

impl Decoder for BatCodec {
    type Item = BatFrame;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<BatFrame>, io::Error> {
        match self.state {
            State::Text => {
                if buf.len() > 0 {
                    if let Some(offset) = buf[..].iter().position(|b| *b == BYTE_ESC) {
                        let bytes = buf.split_to(offset);
                        buf.split_to(1);
                        let frame = self.process(bytes);
                        self.transition_to(State::Esc);
                        Ok(Some(frame))
                    } else {
                        let len = buf.len();
                        let frame = self.process(buf.split_to(len));
                        Ok(Some(frame))
                    }
                } else {
                    Ok(None)
                }
            },

            State::Esc => {
                if buf.len() > 0 {
                    match buf[0] {
                        BYTE_OPEN => {
                            buf.split_to(1);
                            self.transition_to(State::Open(None, None));
                            Ok(Some(BatFrame::Nothing))
                        },

                        BYTE_CLOSE => {
                            buf.split_to(1);
                            self.transition_to(State::Close(None, None));
                            Ok(Some(BatFrame::Nothing))
                        },

                        BYTE_PIPE => {
                            buf.split_to(1);
                            self.transition_to(State::Text);

                            match self.code {
                                Some(ref mut code) => {
                                    code.attr = code.body.clone();
                                    code.body.clear();
                                    Ok(Some(BatFrame::Nothing))
                                },

                                None => {
                                    let frame = self.process(BytesMut::from(&[BYTE_ESC, BYTE_PIPE][..]));
                                    Ok(Some(frame))
                                },
                            }
                        },

                        _ => {
                            let frame = self.process(BytesMut::from(&[BYTE_ESC][..]));
                            self.transition_to(State::Text);
                            Ok(Some(frame))
                        },
                    }
                } else {
                    Ok(None)
                }
            },

            State::Open(None, None) => {
                if buf.len() > 0 {
                    match buf[0] {
                        c1 @ b'0' ..= b'9' => {
                            buf.split_to(1);
                            self.transition_to(State::Open(Some(c1), None));
                            Ok(Some(BatFrame::Nothing))
                        },

                        _ => {
                            let frame = self.process(BytesMut::from(&[BYTE_ESC, BYTE_OPEN][..]));
                            self.transition_to(State::Text);
                            Ok(Some(frame))
                        },
                    }
                } else {
                    Ok(None)
                }
            },

            State::Open(Some(c1), None) => {
                if buf.len() > 0 {
                    match buf[0] {
                        c2 @ b'0' ..= b'9' => {
                            buf.split_to(1);
                            self.transition_to(State::Open(Some(c1), Some(c2)));
                            Ok(Some(BatFrame::Nothing))
                        },

                        _ => {
                            let frame = self.process(BytesMut::from(&[BYTE_ESC, BYTE_OPEN, c1][..]));
                            self.transition_to(State::Text);
                            Ok(Some(frame))
                        },
                    }
                } else {
                    Ok(None)
                }
            },

            State::Open(Some(c1), Some(c2)) => {
                if buf.len() > 0 {
                    let current_code: Option<Box<ControlCode>> = self.code.clone();
                    let new_code = ControlCode::new((c1, c2), current_code.map(|x| *x));
                    self.code = Some(Box::new(new_code));
                    self.transition_to(State::Text);
                    Ok(Some(BatFrame::Nothing))
                } else {
                    Ok(None)
                }
            },

            State::Close(None, None) => {
                if buf.len() > 0 {
                    match buf[0] {
                        c1 @ b'0' ..= b'9' => {
                            buf.split_to(1);
                            self.transition_to(State::Close(Some(c1), None));
                            Ok(Some(BatFrame::Nothing))
                        },

                        _ => {
                            let frame = self.process(BytesMut::from(&[BYTE_ESC, BYTE_CLOSE][..]));
                            self.transition_to(State::Text);
                            Ok(Some(frame))
                        },
                    }
                } else {
                    Ok(None)
                }
            },

            State::Close(Some(c1), None) => {
                if buf.len() > 0 {
                    match buf[0] {
                        c2 @ b'0' ..= b'9' => {
                            buf.split_to(1);
                            self.transition_to(State::Close(Some(c1), Some(c2)));
                            Ok(Some(BatFrame::Nothing))
                        },

                        _ => {
                            let frame = self.process(BytesMut::from(&[BYTE_ESC, BYTE_CLOSE, c1][..]));
                            self.transition_to(State::Text);
                            Ok(Some(frame))
                        },
                    }
                } else {
                    Ok(None)
                }
            },

            State::Close(Some(c1), Some(c2)) => {
                match self.code.clone() {
                    Some(ref code) if code.id == (c1, c2) => {
                        match code.parent.clone() {
                            Some(mut parent) => {
                                let child_bytes = code.to_bytes();
                                self.code = Some(parent);
                                let frame = self.process(child_bytes);
                                self.transition_to(State::Text);
                                Ok(Some(frame))
                            },

                            None => {
                                let frame = BatFrame::Code(code.clone());
                                self.code = None;
                                self.transition_to(State::Text);
                                Ok(Some(frame))
                            }
                        }
                    },

                    _ => {
                        debug!("discard unmatching close code: {}{}", c1, c2);
                        self.transition_to(State::Text);
                        Ok(Some(BatFrame::Nothing))
                    },
                }
            },

            _ => Ok(Some(BatFrame::Nothing)),
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

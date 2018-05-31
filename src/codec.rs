use std::{io, fmt};

use bytes::{Bytes, BufMut, BytesMut};
use tokio_io::codec::{Encoder, Decoder};

use super::color::rgb_to_xterm;

/// Batmud BC mode codec
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

#[derive(Clone, Debug)]
pub struct ControlCode {
    id: (u8, u8),
    attr: BytesMut,
    body: BytesMut,
    parent: Option<Box<ControlCode>>,
}

impl ControlCode {
    fn new(id: (u8, u8), parent: Option<ControlCode>) -> ControlCode {
        ControlCode {
            id: id,
            attr: BytesMut::new(),
            body: BytesMut::new(),
            parent: parent.map(Box::new),
        }
    }
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

fn u8_to_chars(mut x: u8) -> BytesMut {
    let mut bytes = BytesMut::with_capacity(3);
    while x >= 10 {
        bytes.put(x % 10 + b'0');
        x = x / 10;
    }
    bytes.put(x + b'0');
    bytes.reverse();
    bytes
}

fn relay_info_prefix<'a>((c1, c2): (u8, u8)) -> &'a[u8] {
    match (c1, c2) {
        (b'4', b'1') => &b"[player_spell_action_status] "[..],
        (b'4', b'2') => &b"[player_skill_action_status] "[..],
        (b'5', b'0') => &b"[player_full_health_status] "[..],
        (b'5', b'1') => &b"[player_partial_health_status] "[..],
        (b'5', b'2') => &b"[player_info] "[..],
        (b'5', b'3') => &b"[player_free_exp] "[..],
        (b'5', b'4') => &b"[player_status] "[..],
        (b'6', b'0') => &b"[player_location] "[..],
        (b'6', b'1') => &b"[player_party_position] "[..],
        (b'6', b'2') => &b"[party_player_status] "[..],
        (b'6', b'3') => &b"[party_player_left] "[..],
        (b'6', b'4') => &b"[player_effect] "[..],
        (b'7', b'0') => &b"[player_target] "[..],
        (b'9', b'9') => &b"[custom_info] "[..],
        _            => &b"[unspecified] "[..],
    }
}

impl BatCodec {
    pub fn new() -> BatCodec {
        BatCodec {
            state: State::Text,
            next_index: 0,
            code: None,
        }
    }

    fn process(&mut self, bytes: BytesMut) -> Option<BytesMut> {
        match self.code {
            Some(ref mut code) => {
                code.body.reserve(bytes.len());
                code.body.put(bytes);
                Some(BytesMut::new())
            },

            _ => {
                Some(bytes)
            },
        }
    }

    fn transition_to(&mut self, state: State) {
        let id = self.code.clone().map_or(('-', '-'), |c| (c.id.0 as char, c.id.1 as char));
        debug!("State transition from {} to {}. Current code: {}{}", self.state, state, id.0, id.1);
        self.state = state;
    }

    fn process_code(&mut self) -> BytesMut {
        match self.code.clone() {
            Some(code) => match code.id {
                (b'0', b'0') => {
                    // Closes any open control code tags and resets text properties
                    // ESC<00ESC>00
                    self.code = None;
                    BytesMut::from(&"\x1b[0m"[..])
                },

                (b'0', b'5') => {
                    // Signifies that the connection was successful
                    // ESC<05ESC>05
                    self.code = code.parent;
                    BytesMut::from(&"[login] OK\n"[..])
                },

                (b'0', b'6') => {
                    // Signifies that the connection failed with the reason given as arg
                    // ESC<06Incorrect password.ESC>06
                    self.code = code.parent.clone();
                    let mut bytes = BytesMut::with_capacity(10 + code.body.len());
                    bytes.put(&b"[login] "[..]);
                    bytes.put(code.body);
                    bytes.put(b'\n');
                    bytes
                },

                (b'1', b'0') => {
                    // Defines the output to be a message of type <arg>
                    // ESC<10chan_salesESC|Test outputESC>10
                    self.code = code.parent.clone();
                    let mut bytes = BytesMut::with_capacity(4 + code.attr.len() + code.body.len());
                    bytes.put(b'[');
                    bytes.put(code.attr.clone());
                    bytes.put(&b"] "[..]);
                    bytes.put(code.body);
                    bytes.put(b'\n');
                    bytes
                },

                (b'1', b'1') => {
                    // Clears the active screen
                    // ESC<11ESC>11
                    self.code = code.parent;
                    BytesMut::from(&"[clear_screen]\n"[..])
                },

                (b'2', b'0') => {
                    // Sets the text foreground color to be the RGB value specified as argument
                    // ESC<2000FFFFESC|TestESC>20
                    self.code = code.parent.clone();
                    let mut bytes = BytesMut::with_capacity(15 + code.body.len());
                    bytes.put(&b"\x1b[38;5;"[..]);
                    bytes.put(u8_to_chars(rgb_to_xterm(code.attr.clone().freeze())));
                    bytes.put(b'm');
                    bytes.put(code.body);
                    bytes.put(&b"\x1b[0m"[..]);
                    bytes
                },

                (b'2', b'1') => {
                    // Sets the text background color to be the RGB value specified as argument
                    // ESC<21FF0000ESC|TestESC>21
                    self.code = code.parent.clone();
                    let mut bytes = BytesMut::with_capacity(15 + code.body.len());
                    bytes.put(&b"\x1b[48;5;"[..]);
                    bytes.put(u8_to_chars(rgb_to_xterm(code.attr.clone().freeze())));
                    bytes.put(b'm');
                    bytes.put(code.body);
                    bytes.put(&b"\x1b[0m"[..]);
                    bytes
                },

                (b'2', b'2') => {
                    // Sets the text output to bold mode
                    // ESC<22TestESC>22
                    self.code = code.parent.clone();
                    let mut bytes = BytesMut::with_capacity(8 + code.body.len());
                    bytes.put(&b"\x1b[1m"[..]);
                    bytes.put(code.body);
                    bytes.put(&b"\x1b[0m"[..]);
                    bytes
                },

                (b'2', b'3') => {
                    // Sets the text output in italic
                    // ESC<23TestESC>23
                    self.code = code.parent.clone();
                    let mut bytes = BytesMut::with_capacity(8 + code.body.len());
                    bytes.put(&b"\x1b[3m"[..]);
                    bytes.put(code.body);
                    bytes.put(&b"\x1b[0m"[..]);
                    bytes
                },

                (b'2', b'4') => {
                    // Sets the text output as underlined
                    // ESC<24TestESC>24
                    self.code = code.parent.clone();
                    let mut bytes = BytesMut::with_capacity(8 + code.body.len());
                    bytes.put(&b"\x1b[4m"[..]);
                    bytes.put(code.body);
                    bytes.put(&b"\x1b[0m"[..]);
                    bytes
                },

                (b'2', b'5') => {
                    // Sets the text output to blink
                    // ESC<25TestESC>25
                    self.code = code.parent.clone();
                    let mut bytes = BytesMut::with_capacity(8 + code.body.len());
                    bytes.put(&b"\x1b[5m"[..]);
                    bytes.put(code.body);
                    bytes.put(&b"\x1b[0m"[..]);
                    bytes
                },

                (b'2', b'9') => {
                    // Resets the text properties (reverts back to default colors)
                    // ESC<29ESC>29
                    self.code = code.parent.clone();
                    BytesMut::from(&b"\x1b[0m"[..])
                },

                (b'3', b'0') => {
                    // Sets the text to be a hyperlink to the link provides as argument
                    // ESC<30http://www.bat.orgESC|BatMUD's homepageESC>30
                    self.code = code.parent.clone();
                    let mut bytes = BytesMut::with_capacity(4 + code.attr.len() + code.body.len());
                    bytes.put(b'[');
                    bytes.put(code.body.clone());
                    bytes.put(&b"]("[..]);
                    bytes.put(code.attr);
                    bytes.put(b')');
                    bytes
                },

                (b'3', b'1') => {
                    // Sets the text to be an in-game link as provided by argument
                    // ESC<31northESC|Go northESC>31
                    self.code = code.parent.clone();
                    let mut bytes = BytesMut::with_capacity(4 + code.attr.len() + code.body.len());
                    bytes.put(b'[');
                    bytes.put(code.body.clone());
                    bytes.put(&b"]("[..]);
                    bytes.put(code.attr);
                    bytes.put(b')');
                    bytes
                },

                (b'4', b'0') => {
                    // Clears spell/skill progress indicator
                    // ESC<40ESC>40
                    self.code = code.parent.clone();
                    BytesMut::from(&b"[clear_spell_skill_progress_indicator]\n"[..])
                },

                (c1, c2) => {
                    self.code = code.parent.clone();
                    let prefix = relay_info_prefix((c1, c2));
                    let mut bytes = BytesMut::with_capacity(prefix.len() + code.body.len() + 1);
                    bytes.put(&prefix[..]);
                    bytes.put(code.body);
                    bytes.put(b'\n');
                    bytes
                }
            },

            None => BytesMut::new(),
        }
    }
}

const BYTE_ESC: u8 = b'\x1b';
const BYTE_OPEN: u8 = b'<';
const BYTE_CLOSE: u8 = b'>';
const BYTE_PIPE: u8 = b'|';

impl Decoder for BatCodec {
    type Item = BytesMut;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<BytesMut>, io::Error> {
        match self.state {
            State::Text => {
                if buf.len() > 0 {
                    if let Some(offset) = buf[..].iter().position(|b| *b == BYTE_ESC) {
                        let bytes = buf.split_to(offset);
                        buf.split_to(1);
                        self.transition_to(State::Esc);
                        Ok(self.process(bytes))
                    } else {
                        let len = buf.len();
                        Ok(self.process(buf.split_to(len)))
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
                            Ok(Some(BytesMut::new()))
                        },

                        BYTE_CLOSE => {
                            buf.split_to(1);
                            self.transition_to(State::Close(None, None));
                            Ok(Some(BytesMut::new()))
                        },

                        BYTE_PIPE => {
                            buf.split_to(1);
                            self.transition_to(State::Text);

                            match self.code {
                                Some(ref mut code) => {
                                    code.attr = code.body.clone();
                                    code.body.clear();
                                    Ok(Some(BytesMut::new()))
                                },

                                None => {
                                    Ok(Some(BytesMut::from(&b"\x1b|"[..])))
                                },
                            }
                        },

                        _ => {
                            self.transition_to(State::Text);
                            Ok(self.process(BytesMut::from(&[BYTE_ESC][..])))
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
                            Ok(Some(BytesMut::new()))
                        },

                        _ => {
                            self.transition_to(State::Text);
                            Ok(self.process(BytesMut::from(&[BYTE_ESC, BYTE_OPEN][..])))
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
                            Ok(Some(BytesMut::new()))
                        },

                        _ => {
                            self.transition_to(State::Text);
                            Ok(self.process(BytesMut::from(&[BYTE_ESC, BYTE_OPEN, c1][..])))
                        },
                    }
                } else {
                    Ok(None)
                }
            },

            State::Open(Some(c1), Some(c2)) => {
                if buf.len() > 0 {
                    let code = ControlCode::new((c1, c2), self.code.clone().map(|x| *x));
                    self.code = Some(Box::new(code));
                    self.transition_to(State::Text);
                    Ok(Some(BytesMut::new()))
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
                            Ok(Some(BytesMut::new()))
                        },

                        _ => {
                            self.transition_to(State::Text);
                            Ok(self.process(BytesMut::from(&[BYTE_ESC, BYTE_CLOSE][..])))
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
                            Ok(Some(BytesMut::new()))
                        },

                        _ => {
                            self.transition_to(State::Text);
                            Ok(self.process(BytesMut::from(&[BYTE_ESC, BYTE_CLOSE, c1][..])))
                        },
                    }
                } else {
                    Ok(None)
                }
            },

            State::Close(Some(c1), Some(c2)) => {
                self.transition_to(State::Text);
                match self.code.clone() {
                    Some(ref code) if code.id == (c1, c2) => {
                        let bytes = self.process_code();
                        Ok(self.process(bytes))
                    },

                    Some(_) => {
                        // unmatching close tag, discard "ESC>??"
                        Ok(Some(BytesMut::new()))
                    },

                    None => {
                        // should not happen, discard "ESC>??"
                        Ok(Some(BytesMut::new()))
                    },
                }
            },

            _ => Ok(Some(BytesMut::new())),
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

    macro_rules! decode {
        ( $bytes:expr, $expected:expr ) => {{
            let mut bytes = BytesMut::from(&$bytes[..]);
            let mut bat_codec = BatCodec::new();

            let mut output = BytesMut::new();
            loop {
                match bat_codec.decode(&mut bytes) {
                    Ok(Some(bytes)) => {
                        if bytes.len() > 0 {
                            output.reserve(bytes.len());
                            output.put(bytes);
                        }
                    },

                    Ok(None) => {
                        break;
                    },

                    Err(e) => {
                        error!("{}", e);
                        break;
                    }
                }
            }

            assert_eq!(&output[..], &$expected[..]);
        }};
    }

    #[test]
    fn stack() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<20FFFFFF\x1b|\x1b<210000FF\x1b|Test output, white on blue\x1b>21\x1b>20",
            b"\x1b[38;5;15m\x1b[48;5;12mTest output, white on blue\x1b[0m\x1b[0m"
        );
    }

    #[test]
    fn stack2() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<10chan_swe+\x1b|\x1b<20FFFF00\x1b|Gore <swe+>: detta \x84r n\x86got p\x86 svenska\x1b>20\x1b>10",
            b"[chan_swe+] \x1b[38;5;11mGore <swe+>: detta \x84r n\x86got p\x86 svenska\x1b[0m\n"
        );
    }

    #[test]
    fn code_00() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<00\x1b>00",
            b"\x1b[0m"
        );
    }

    #[test]
    fn code_05() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<05\x1b>05",
            b"[login] OK\n"
        );
    }

    #[test]
    fn code_06() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<06Incorrect password.\x1b>06",
            b"[login] Incorrect password.\n"
        );
    }

    #[test]
    fn code_10() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<10chan_sales\x1b|Test output\x1b>10",
            b"[chan_sales] Test output\n"
        );
    }

    #[test]
    fn code_11() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<11\x1b>11",
            b"[clear_screen]\n"
        );
    }

    #[test]
    fn code_20() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<20FF0000\x1b|Test\x1b>20",
            b"\x1b[38;5;9mTest\x1b[0m"
        );
    }

    #[test]
    fn code_21() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<21d70000\x1b|Test\x1b>21",
            b"\x1b[48;5;160mTest\x1b[0m"
        );
    }

    #[test]
    fn code_22() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<22Test\x1b>22",
            b"\x1b[1mTest\x1b[0m"
        );
    }

    #[test]
    fn code_23() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<23Test\x1b>23",
            b"\x1b[3mTest\x1b[0m"
        );
    }

    #[test]
    fn code_24() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<24Test\x1b>24",
            b"\x1b[4mTest\x1b[0m"
        );
    }

    #[test]
    fn code_25() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<25Test\x1b>25",
            b"\x1b[5mTest\x1b[0m"
        );
    }

    #[test]
    fn code_29() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<29\x1b>29",
            b"\x1b[0m"
        );
    }

    #[test]
    fn code_30() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<30http://bat.org\x1b|BatMUD\x1b>30",
            b"[BatMUD](http://bat.org)"
        );
    }

    #[test]
    fn code_31() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<31north\x1b|Go north\x1b>31",
            b"[Go north](north)"
        );
    }

    #[test]
    fn code_40() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<40\x1b>40",
            b"[clear_spell_skill_progress_indicator]\n"
        );
    }

    #[test]
    fn code_41() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<41magic_missile 2\x1b>41",
            b"[player_spell_action_status] magic_missile 2\n"
        );
    }

    #[test]
    fn code_42() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<42bladed_fury 5\x1b>42",
            b"[player_skill_action_status] bladed_fury 5\n"
        );
    }

    #[test]
    fn code_50() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<50100 200 200 250 300 350\x1b>50",
            b"[player_full_health_status] 100 200 200 250 300 350\n"
        );
    }

    #[test]
    fn code_51() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<51100 200 200\x1b>51",
            b"[player_partial_health_status] 100 200 200\n"
        );
    }

    #[test]
    fn code_52() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<52Ulath Pulath coder 100 1 1345323\x1b>52",
            b"[player_info] Ulath Pulath coder 100 1 1345323\n"
        );
    }

    #[test]
    fn code_53() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<531345323\x1b>53",
            b"[player_free_exp] 1345323\n"
        );
    }

    #[test]
    fn code_54() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<540 0 0\x1b>54",
            b"[player_status] 0 0 0\n"
        );
    }

    #[test]
    fn code_60() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<60ulath coder 1 laenor 5100 5200 0\x1b>60",
            b"[player_location] ulath coder 1 laenor 5100 5200 0\n"
        );
    }

    #[test]
    fn code_61() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<61ulath 1 1\x1b>61",
            b"[player_party_position] ulath 1 1\n"
        );
    }

    #[test]
    fn code_62() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<62Killer orc 1 50 101 200 202 303 404 504 ekuva_ja_expaa 1 1 1 0 0 0 0 1 0 0 0 0 0 0 0 12345 100000 1234 Wed_Oct_31_15:57:52_2007\x1b>62",
            b"[party_player_status] Killer orc 1 50 101 200 202 303 404 504 ekuva_ja_expaa 1 1 1 0 0 0 0 1 0 0 0 0 0 0 0 12345 100000 1234 Wed_Oct_31_15:57:52_2007\n"
        );
    }

    #[test]
    fn code_63() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<63ulath\x1b>63",
            b"[party_player_left] ulath\n"
        );
    }

    #[test]
    fn code_64() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<64lay_on_hands 120\x1b>64",
            b"[player_effect] lay_on_hands 120\n"
        );
    }

    #[test]
    fn code_70() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<70evilmonster 45\x1b>70",
            b"[player_target] evilmonster 45\n"
        );
    }

    #[test]
    fn code_99() {
        let _ = env_logger::try_init();
        decode!(
            b"\x1b<991 dex 300\x1b>99",
            b"[custom_info] 1 dex 300\n"
        );
    }
}

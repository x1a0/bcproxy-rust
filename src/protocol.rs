use bytes::{BufMut, BytesMut};

use super::color::rgb_to_xterm;

#[derive(Clone, Debug)]
pub struct ControlCode {
    pub id: (u8, u8),
    pub attr: BytesMut,
    pub body: BytesMut,
    pub parent: Option<Box<ControlCode>>,
    pub closed_child: Option<Box<ControlCode>>,
}

macro_rules! relay_prefix {
    ($id:expr) => {
        match $id {
            (b'4', b'0') => &b"[player_action_indicator_clear]"[..],
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
    };
}

impl ControlCode {
    pub fn new(id: (u8, u8), parent: Option<ControlCode>) -> ControlCode {
        ControlCode {
            id: id,
            attr: BytesMut::new(),
            body: BytesMut::new(),
            parent: parent.map(Box::new),
            closed_child: None,
        }
    }

    pub fn to_bytes(&self) -> BytesMut {
        let mut body = self.body.clone();

        if let Some(ref code) = self.closed_child {
            let bytes = code.to_bytes();
            let len = bytes.len();
            body.reserve(len);
            body.put(bytes);
        }

        match self.id {
            (b'0', b'0') => {
                // Closes any open control code tags and resets text properties
                // ESC<00ESC>00
                BytesMut::from(&"\x1b[0m"[..])
            },

            (b'0', b'5') => {
                // Signifies that the connection was successful
                // ESC<05ESC>05
                BytesMut::from(&"[login] OK\n"[..])
            },

            (b'0', b'6') => {
                // Signifies that the connection failed with the reason given as arg
                // ESC<06Incorrect password.ESC>06
                let mut bytes = BytesMut::with_capacity(10 + body.len());
                bytes.put(&b"[login] "[..]);
                bytes.put(body);
                bytes.put(b'\n');
                bytes
            },

            (b'1', b'0') => {
                // Defines the output to be a message of type <arg>
                // ESC<10chan_salesESC|Test outputESC>10
                let mut bytes = BytesMut::with_capacity(4 + self.attr.len() + body.len());
                bytes.put(b'[');
                bytes.put(self.attr.clone());
                bytes.put(&b"] "[..]);
                bytes.put(body);
                bytes.put(b'\n');
                bytes
            },

            (b'1', b'1') => {
                // Clears the active screen
                // ESC<11ESC>11
                BytesMut::from(&"[clear_screen]\n"[..])
            },

            (b'2', b'0') => {
                // Sets the text foreground color to be the RGB value specified as argument
                // ESC<2000FFFFESC|TestESC>20
                let mut bytes = BytesMut::with_capacity(15 + body.len());
                bytes.put(&b"\x1b[38;5;"[..]);
                bytes.put(u8_to_chars(rgb_to_xterm(self.attr.clone())));
                bytes.put(b'm');
                bytes.put(body);
                bytes.put(&b"\x1b[0m"[..]);
                bytes
            },

            (b'2', b'1') => {
                // Sets the text background color to be the RGB value specified as argument
                // ESC<21FF0000ESC|TestESC>21
                let mut bytes = BytesMut::with_capacity(15 + body.len());
                bytes.put(&b"\x1b[48;5;"[..]);
                bytes.put(u8_to_chars(rgb_to_xterm(self.attr.clone())));
                bytes.put(b'm');
                bytes.put(body);
                bytes.put(&b"\x1b[0m"[..]);
                bytes
            },

            (b'2', b'2') => {
                // Sets the text output to bold mode
                // ESC<22TestESC>22
                let mut bytes = BytesMut::with_capacity(8 + body.len());
                bytes.put(&b"\x1b[1m"[..]);
                bytes.put(body);
                bytes.put(&b"\x1b[0m"[..]);
                bytes
            },

            (b'2', b'3') => {
                // Sets the text output in italic
                // ESC<23TestESC>23
                let mut bytes = BytesMut::with_capacity(8 + body.len());
                bytes.put(&b"\x1b[3m"[..]);
                bytes.put(body);
                bytes.put(&b"\x1b[0m"[..]);
                bytes
            },

            (b'2', b'4') => {
                // Sets the text output as underlined
                // ESC<24TestESC>24
                let mut bytes = BytesMut::with_capacity(8 + body.len());
                bytes.put(&b"\x1b[4m"[..]);
                bytes.put(body);
                bytes.put(&b"\x1b[0m"[..]);
                bytes
            },

            (b'2', b'5') => {
                // Sets the text output to blink
                // ESC<25TestESC>25
                let mut bytes = BytesMut::with_capacity(8 + body.len());
                bytes.put(&b"\x1b[5m"[..]);
                bytes.put(body);
                bytes.put(&b"\x1b[0m"[..]);
                bytes
            },

            (b'2', b'9') => {
                // Resets the text properties (reverts back to default colors)
                // ESC<29ESC>29
                BytesMut::from(&b"\x1b[0m"[..])
            },

            (b'3', b'0') => {
                // Sets the text to be a hyperlink to the link provides as argument
                // ESC<30http://www.bat.orgESC|BatMUD's homepageESC>30
                let mut bytes = BytesMut::with_capacity(4 + self.attr.len() + body.len());
                bytes.put(b'[');
                bytes.put(body);
                bytes.put(&b"]("[..]);
                bytes.put(self.attr.clone());
                bytes.put(b')');
                bytes
            },

            (b'3', b'1') => {
                // Sets the text to be an in-game link as provided by argument
                // ESC<31northESC|Go northESC>31
                if body == self.attr {
                    let mut bytes = BytesMut::with_capacity(8 + body.len());
                    bytes.put(&b"\x1b[4m"[..]);
                    bytes.put(body);
                    bytes.put(&b"\x1b[0m"[..]);
                    bytes
                } else {
                    let mut bytes = BytesMut::with_capacity(4 + self.attr.len() + body.len());
                    bytes.put(b'[');
                    bytes.put(body);
                    bytes.put(&b"]("[..]);
                    bytes.put(self.attr.clone());
                    bytes.put(b')');
                    bytes
                }
            },

            (c1, c2) => {
                let prefix = relay_prefix!((c1, c2));
                let mut bytes = BytesMut::with_capacity(prefix.len() + body.len() + 1);
                bytes.put(&prefix[..]);
                bytes.put(body);
                bytes.put(b'\n');
                bytes
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use env_logger;

    macro_rules! mk_code {
        ($code:expr) => {
            ControlCode::new($code, None)
        };

        ($code:expr, $body:expr) => {
            {
                let mut code = mk_code!($code);
                code.body.extend_from_slice(&$body[..]);
                code
            }
        };

        ($code:expr, $body:expr, $attr:expr) => {
            {
                let mut code = mk_code!($code, $body);
                code.attr.extend_from_slice(&$attr[..]);
                code
            }
        };

        ($code:expr, $body:expr, $attr:expr, $closed_child:expr) => {
            {
                let mut parent = mk_code!($code, $body, $attr);
                parent.closed_child = Some(Box::new($closed_child));
                parent
            }
        };
    }

    #[test]
    fn code_00() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'0', b'0'));
        assert_eq!(&code.to_bytes()[..], b"\x1b[0m");
    }

    #[test]
    fn code_05() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'0', b'5'));
        assert_eq!(&code.to_bytes()[..], b"[login] OK\n");
    }

    #[test]
    fn code_06() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'0', b'6'), b"Incorrect password.");
        assert_eq!(&code.to_bytes()[..], b"[login] Incorrect password.\n");
    }

    #[test]
    fn code_10() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'1', b'0'), b"Test output", b"chan_sales");
        assert_eq!(&code.to_bytes()[..], b"[chan_sales] Test output\n");
    }

    #[test]
    fn code_11() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'1', b'1'));
        assert_eq!(&code.to_bytes()[..], b"[clear_screen]\n");
    }

    #[test]
    fn code_20() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'2', b'0'), b"Test", b"00FFFF");
        assert_eq!(&code.to_bytes()[..], b"\x1b[38;5;14mTest\x1b[0m");
    }

    #[test]
    fn code_21() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'2', b'1'), b"Test", b"FF0000");
        assert_eq!(&code.to_bytes()[..], b"\x1b[48;5;9mTest\x1b[0m");
    }

    #[test]
    fn code_22() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'2', b'2'), b"Test");
        assert_eq!(&code.to_bytes()[..], b"\x1b[1mTest\x1b[0m");
    }

    #[test]
    fn code_23() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'2', b'3'), b"Test");
        assert_eq!(&code.to_bytes()[..], b"\x1b[3mTest\x1b[0m");
    }

    #[test]
    fn code_24() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'2', b'4'), b"Test");
        assert_eq!(&code.to_bytes()[..], b"\x1b[4mTest\x1b[0m");
    }

    #[test]
    fn code_25() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'2', b'5'), b"Test");
        assert_eq!(&code.to_bytes()[..], b"\x1b[5mTest\x1b[0m");
    }

    #[test]
    fn code_29() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'2', b'9'));
        assert_eq!(&code.to_bytes()[..], b"\x1b[0m");
    }

    #[test]
    fn code_30() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'3', b'0'), b"BatMUD's homepage", b"http://www.bat.org");
        assert_eq!(&code.to_bytes()[..], &b"[BatMUD's homepage](http://www.bat.org)"[..]);
    }

    #[test]
    fn code_31_different() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'3', b'1'), b"Go north", b"north");
        assert_eq!(&code.to_bytes()[..], b"[Go north](north)");
    }

    #[test]
    fn code_31_same() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'3', b'1'), b"north", b"north");
        assert_eq!(&code.to_bytes()[..], b"\x1b[4mnorth\x1b[0m");
    }

    #[test]
    fn code_40() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'4', b'0'));
        assert_eq!(&code.to_bytes()[..], b"[player_action_indicator_clear]\n");
    }

    #[test]
    fn code_41() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'4', b'1'), b"magic_missile 2");
        assert_eq!(&code.to_bytes()[..], &b"[player_spell_action_status] magic_missile 2\n"[..]);
    }

    #[test]
    fn code_42() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'4', b'2'), b"bladed_fury 5");
        assert_eq!(&code.to_bytes()[..], &b"[player_skill_action_status] bladed_fury 5\n"[..]);
    }

    #[test]
    fn code_50() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'5', b'0'), b"100 200 200 250 300 350");
        assert_eq!(&code.to_bytes()[..], &b"[player_full_health_status] 100 200 200 250 300 350\n"[..]);
    }

    #[test]
    fn code_51() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'5', b'1'), b"100 200 200");
        assert_eq!(&code.to_bytes()[..], &b"[player_partial_health_status] 100 200 200\n"[..]);
    }

    #[test]
    fn code_52() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'5', b'2'), b"Ulath Pulath coder 100 1 1345323");
        assert_eq!(&code.to_bytes()[..], &b"[player_info] Ulath Pulath coder 100 1 1345323\n"[..]);
    }

    #[test]
    fn code_53() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'5', b'3'), b"531345323");
        assert_eq!(&code.to_bytes()[..], &b"[player_free_exp] 531345323\n"[..]);
    }

    #[test]
    fn code_54() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'5', b'4'), b"0 0 0");
        assert_eq!(&code.to_bytes()[..], &b"[player_status] 0 0 0\n"[..]);
    }

    #[test]
    fn code_60() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'6', b'0'), b"ulath coder 1 laenor 5100 5200 0");
        assert_eq!(&code.to_bytes()[..], &b"[player_location] ulath coder 1 laenor 5100 5200 0\n"[..]);
    }

    #[test]
    fn code_61() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'6', b'1'), b"ulath 1 1");
        assert_eq!(&code.to_bytes()[..], &b"[player_party_position] ulath 1 1\n"[..]);
    }

    #[test]
    fn code_62() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'6', b'2'), b"Killer orc 1 50 101 200 202 303 404 504 ekuva_ja_expaa 1 1 1 0 0 0 0 1 0 0 0 0 0 0 0 12345 100000 1234 Wed_Oct_31_15:57:52_2007");
        assert_eq!(&code.to_bytes()[..], &b"[party_player_status] Killer orc 1 50 101 200 202 303 404 504 ekuva_ja_expaa 1 1 1 0 0 0 0 1 0 0 0 0 0 0 0 12345 100000 1234 Wed_Oct_31_15:57:52_2007\n"[..]);
    }

    #[test]
    fn code_63() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'6', b'3'), b"ulath");
        assert_eq!(&code.to_bytes()[..], &b"[party_player_left] ulath\n"[..]);
    }

    #[test]
    fn code_64() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'6', b'4'), b"lay_on_hands 120");
        assert_eq!(&code.to_bytes()[..], &b"[player_effect] lay_on_hands 120\n"[..]);
    }

    #[test]
    fn code_70() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'7', b'0'), b"evilmonster 45");
        assert_eq!(&code.to_bytes()[..], &b"[player_target] evilmonster 45\n"[..]);
    }

    #[test]
    fn code_99() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'9', b'9'), b"1 dex 300");
        assert_eq!(&code.to_bytes()[..], &b"[custom_info] 1 dex 300\n"[..]);
    }

    #[test]
    fn code_stack() {
        let _ = env_logger::try_init();
        let child = mk_code!((b'2', b'1'), b"Test output, white on blue", b"0000FF");
        let code = mk_code!((b'2', b'0'), b"", b"FFFFFF", child);
        assert_eq!(&code.to_bytes()[..], &b"\x1b[38;5;15m\x1b[48;5;12mTest output, white on blue\x1b[0m\x1b[0m"[..]);
    }
}

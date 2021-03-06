use bytes::{BufMut, BytesMut};

use super::super::color::rgb_to_xterm;
use super::usize_to_chars;

#[derive(Clone, Debug)]
pub struct ControlCode {
    pub id: (u8, u8),
    pub attr: BytesMut,
    pub body: BytesMut,
    pub parent: Option<Box<ControlCode>>,
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
            _            => &b"[unspecified] "[..],
        }
    };
}

fn count_lines(bytes: &BytesMut) -> usize {
    let line_break_found = bytes.iter().fold(0, |acc, x| match x {
        b'\n' => acc + 1,
        _ => acc
    });

    match bytes.last() {
        Some(b) if *b != b'\n' => line_break_found + 1,
        _ => line_break_found,
    }
}

impl ControlCode {
    pub fn new(id: (u8, u8), parent: Option<Box<ControlCode>>) -> ControlCode {
        ControlCode {
            id: id,
            attr: BytesMut::new(),
            body: BytesMut::new(),
            parent: parent,
        }
    }

    pub fn to_bytes(&self) -> BytesMut {
        let mut body = self.body.clone();

        match self.id {
            // Closes any open control code tags and resets text properties
            // ESC<00ESC>00
            (b'0', b'0') => {
                BytesMut::from(&"\x1b[0m"[..])
            },

            // Signifies that the connection was successful
            // ESC<05ESC>05
            (b'0', b'5') => {
                BytesMut::from(&"[login] OK\n"[..])
            },

            // Signifies that the connection failed with the reason given as arg
            // ESC<06Incorrect password.ESC>06
            (b'0', b'6') => {
                let mut bytes = BytesMut::with_capacity(10 + body.len());
                bytes.put(&b"[login] "[..]);
                bytes.put(body);
                bytes.put(b'\n');
                bytes
            },

            // Defines the output to be a message of type <arg>
            // ESC<10chan_salesESC|Test outputESC>10
            (b'1', b'0') if &self.attr[..] == b"spec_map" && &body[..] == b"NoMapSupport" => {
                BytesMut::from(&b"[spec_map] NoMapSupport\n"[..])
            },

            (b'1', b'0') if &self.attr[..] == b"spec_map" => {
                // "spec_map" has multiple lines:
                // [spec_map:0] [clear_screen]
                // [spec_map:1] ...
                // [spec_map:2] ...

                let mut lines = count_lines(&body);
                let mut final_len = 0;

                // FIXME: this cannot support lines > 99
                let mut base = 0;
                if lines > 10 {
                    final_len += (5 + base + self.attr.len()) * 10;
                    lines = lines - 10;
                    base += 1;
                }
                final_len += (5 + base + self.attr.len()) * lines;
                final_len += body.len();

                let mut line = 0;
                let mut bytes = BytesMut::with_capacity(final_len);

                while let Some(n) = body[..].iter().position(|b| *b == b'\n') {
                    bytes.put(b'[');
                    bytes.put(&self.attr[..]);
                    bytes.put(b':');

                    if line < 10 {
                        bytes.put(b'0' + line);
                    } else {
                        bytes.put(b'1');
                        bytes.put(b'0' + line - 10);
                    }

                    bytes.put(&b"] "[..]);
                    bytes.put(body.split_to(n + 1));

                    line += 1;
                }

                if body.len() > 0 {
                    bytes.put(b'[');
                    bytes.put(&self.attr[..]);
                    bytes.put(b':');
                    bytes.put(b'0' + line);
                    bytes.put(&b"] "[..]);
                    bytes.put(body);
                }

                bytes
            },

            (b'1', b'0') if &self.attr[..] == b"spec_prompt" => {
                let mut bytes = BytesMut::with_capacity(4 + self.attr.len() + body.len());
                bytes.put(b'[');
                bytes.put(self.attr.clone());
                bytes.put(&b"] "[..]);
                bytes.put(body);
                bytes.put(b'\n');
                bytes
            },

            (b'1', b'0') if self.attr.starts_with(b"chan_") => {
                body
            },

            (b'1', b'0') => {
                let mut bytes = BytesMut::with_capacity(3 + self.attr.len() + body.len());
                bytes.put(b'[');
                bytes.put(self.attr.clone());
                bytes.put(&b"] "[..]);
                bytes.put(body);
                bytes
            },

            // Clears the active screen
            // ESC<11ESC>11
            (b'1', b'1') => {
                BytesMut::from(&"[clear_screen]\n"[..])
            },

            // Sets the text foreground color to be the RGB value specified as argument
            // ESC<2000FFFFESC|TestESC>20
            (b'2', b'0') => {
                let color_bytes = usize_to_chars(rgb_to_xterm(self.attr.clone()) as usize);
                let mut bytes = BytesMut::with_capacity(12 + color_bytes.len() + body.len());
                bytes.put(&b"\x1b[38;5;"[..]);
                bytes.put(color_bytes);
                bytes.put(b'm');
                bytes.put(body);
                bytes.put(&b"\x1b[0m"[..]);
                bytes
            },

            // Sets the text background color to be the RGB value specified as argument
            // ESC<21FF0000ESC|TestESC>21
            (b'2', b'1') => {
                let color_bytes = usize_to_chars(rgb_to_xterm(self.attr.clone()) as usize);
                let mut bytes = BytesMut::with_capacity(12 + color_bytes.len() + body.len());
                bytes.put(&b"\x1b[48;5;"[..]);
                bytes.put(color_bytes);
                bytes.put(b'm');
                bytes.put(body);
                bytes.put(&b"\x1b[0m"[..]);
                bytes
            },

            // Sets the text output to bold mode
            // ESC<22TestESC>22
            (b'2', b'2') => {
                let mut bytes = BytesMut::with_capacity(8 + body.len());
                bytes.put(&b"\x1b[1m"[..]);
                bytes.put(body);
                bytes.put(&b"\x1b[0m"[..]);
                bytes
            },

            // Sets the text output in italic
            // ESC<23TestESC>23
            (b'2', b'3') => {
                let mut bytes = BytesMut::with_capacity(8 + body.len());
                bytes.put(&b"\x1b[3m"[..]);
                bytes.put(body);
                bytes.put(&b"\x1b[0m"[..]);
                bytes
            },

            // Sets the text output as underlined
            // ESC<24TestESC>24
            (b'2', b'4') => {
                let mut bytes = BytesMut::with_capacity(8 + body.len());
                bytes.put(&b"\x1b[4m"[..]);
                bytes.put(body);
                bytes.put(&b"\x1b[0m"[..]);
                bytes
            },

            // Sets the text output to blink
            // ESC<25TestESC>25
            (b'2', b'5') => {
                let mut bytes = BytesMut::with_capacity(8 + body.len());
                bytes.put(&b"\x1b[5m"[..]);
                bytes.put(body);
                bytes.put(&b"\x1b[0m"[..]);
                bytes
            },

            // Resets the text properties (reverts back to default colors)
            // ESC<29ESC>29
            (b'2', b'9') => {
                BytesMut::from(&b"\x1b[0m"[..])
            },

            // Sets the text to be a hyperlink to the link provides as argument
            // ESC<30http://www.bat.orgESC|BatMUD's homepageESC>30
            (b'3', b'0') => {
                let mut bytes = BytesMut::with_capacity(4 + self.attr.len() + body.len());
                bytes.put(b'[');
                bytes.put(body);
                bytes.put(&b"]("[..]);
                bytes.put(self.attr.clone());
                bytes.put(b')');
                bytes
            },

            // Sets the text to be an in-game link as provided by argument
            // ESC<31northESC|Go northESC>31
            (b'3', b'1') => {
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

            (b'9', b'9') if body.starts_with(b"BAT_MAPPER;;") => {
                let mut bytes = BytesMut::from(&b"[bat_mapper] "[..]);
                bytes.extend(body.split_off(12));
                bytes
            },

            (b'9', b'9') => {
                let mut lines = count_lines(&body);
                let mut final_len = 0;
                let prefix = b"custom_info";

                // FIXME: this cannot support lines > 99
                let mut base = 0;
                if lines > 10 {
                    final_len += (5 + base + prefix.len()) * 10;
                    lines = lines - 10;
                    base += 1;
                }
                final_len += (5 + base + prefix.len()) * lines;
                final_len += body.len();
                final_len += 1; // ending \n

                let mut line = 0;
                let mut bytes = BytesMut::with_capacity(final_len);

                while let Some(n) = body[..].iter().position(|b| *b == b'\n') {
                    bytes.put(b'[');
                    bytes.put(&prefix[..]);
                    bytes.put(b':');

                    if line < 10 {
                        bytes.put(b'0' + line);
                    } else {
                        bytes.put(b'1');
                        bytes.put(b'0' + line - 10);
                    }

                    bytes.put(&b"] "[..]);
                    bytes.put(body.split_to(n + 1));

                    line += 1;
                }

                if body.len() > 0 {
                    bytes.put(b'[');
                    bytes.put(&prefix[..]);
                    bytes.put(b':');
                    bytes.put(b'0' + line);
                    bytes.put(&b"] "[..]);
                    bytes.put(body);
                }

                bytes.put(b'\n');
                bytes
            }

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

#[cfg(test)]
mod tests {
    use std::mem::size_of;
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
    }

    macro_rules! verify {
        ($bytes:expr, $expected:expr) => {
            let bytes = $bytes;
            assert_eq!(&bytes[..], &$expected[..]);
            if bytes.len() >= 4 * size_of::<usize>() - 1 {
                assert_eq!(bytes.capacity(), bytes.len());
            }
        }
    }

    #[test]
    fn code_00() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'0', b'0'));
        verify!(code.to_bytes(), b"\x1b[0m");
    }

    #[test]
    fn code_05() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'0', b'5'));
        verify!(code.to_bytes(), b"[login] OK\n");
    }

    #[test]
    fn code_06() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'0', b'6'), b"Incorrect password.");
        verify!(code.to_bytes(), b"[login] Incorrect password.\n");
    }

    #[test]
    fn code_10() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'1', b'0'), b"Test output", b"foo");
        verify!(code.to_bytes(), b"[foo] Test output");
    }

    #[test]
    fn code_10_chan() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'1', b'0'), b"Test output", b"chan_sales");
        verify!(code.to_bytes(), b"Test output");
    }

    #[test]
    fn code_10_spec_prompt() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'1', b'0'), b"Test prompt > ", b"spec_prompt");
        verify!(code.to_bytes(), b"[spec_prompt] Test prompt > \n");
    }

    #[test]
    fn code_10_spec_map() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'1', b'0'), b"1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n", b"spec_map");
        verify!(code.to_bytes(), b"[spec_map:0] 1\n[spec_map:1] 2\n[spec_map:2] 3\n[spec_map:3] 4\n[spec_map:4] 5\n[spec_map:5] 6\n[spec_map:6] 7\n[spec_map:7] 8\n[spec_map:8] 9\n[spec_map:9] 10\n[spec_map:10] 11\n");
    }

    #[test]
    fn code_11() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'1', b'1'));
        verify!(code.to_bytes(), b"[clear_screen]\n");
    }

    #[test]
    fn code_20() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'2', b'0'), b"Test", b"00FFFF");
        verify!(code.to_bytes(), b"\x1b[38;5;14mTest\x1b[0m");
    }

    #[test]
    fn code_21() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'2', b'1'), b"Test", b"FF0000");
        verify!(code.to_bytes(), b"\x1b[48;5;9mTest\x1b[0m");
    }

    #[test]
    fn code_22() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'2', b'2'), b"Test");
        verify!(code.to_bytes(), b"\x1b[1mTest\x1b[0m");
    }

    #[test]
    fn code_23() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'2', b'3'), b"Test");
        verify!(code.to_bytes(), b"\x1b[3mTest\x1b[0m");
    }

    #[test]
    fn code_24() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'2', b'4'), b"Test");
        verify!(code.to_bytes(), b"\x1b[4mTest\x1b[0m");
    }

    #[test]
    fn code_25() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'2', b'5'), b"Test");
        verify!(code.to_bytes(), b"\x1b[5mTest\x1b[0m");
    }

    #[test]
    fn code_29() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'2', b'9'));
        verify!(code.to_bytes(), b"\x1b[0m");
    }

    #[test]
    fn code_30() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'3', b'0'), b"BatMUD's homepage", b"http://www.bat.org");
        verify!(code.to_bytes(), b"[BatMUD's homepage](http://www.bat.org)");
    }

    #[test]
    fn code_31_different() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'3', b'1'), b"Go north", b"north");
        verify!(code.to_bytes(), b"[Go north](north)");
    }

    #[test]
    fn code_31_same() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'3', b'1'), b"north", b"north");
        verify!(code.to_bytes(), b"\x1b[4mnorth\x1b[0m");
    }

    #[test]
    fn code_40() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'4', b'0'));
        verify!(code.to_bytes(), b"[player_action_indicator_clear]\n");
    }

    #[test]
    fn code_41() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'4', b'1'), b"magic_missile 2");
        verify!(code.to_bytes(), b"[player_spell_action_status] magic_missile 2\n");
    }

    #[test]
    fn code_42() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'4', b'2'), b"bladed_fury 5");
        verify!(code.to_bytes(), b"[player_skill_action_status] bladed_fury 5\n");
    }

    #[test]
    fn code_50() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'5', b'0'), b"100 200 200 250 300 350");
        verify!(code.to_bytes(), b"[player_full_health_status] 100 200 200 250 300 350\n");
    }

    #[test]
    fn code_51() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'5', b'1'), b"100 200 200");
        verify!(code.to_bytes(), b"[player_partial_health_status] 100 200 200\n");
    }

    #[test]
    fn code_52() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'5', b'2'), b"Ulath Pulath coder 100 1 1345323");
        verify!(code.to_bytes(), b"[player_info] Ulath Pulath coder 100 1 1345323\n");
    }

    #[test]
    fn code_53() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'5', b'3'), b"531345323");
        verify!(code.to_bytes(), b"[player_free_exp] 531345323\n");
    }

    #[test]
    fn code_54() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'5', b'4'), b"0 0 0");
        verify!(code.to_bytes(), b"[player_status] 0 0 0\n");
    }

    #[test]
    fn code_60() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'6', b'0'), b"ulath coder 1 laenor 5100 5200 0");
        verify!(code.to_bytes(), b"[player_location] ulath coder 1 laenor 5100 5200 0\n");
    }

    #[test]
    fn code_61() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'6', b'1'), b"ulath 1 1");
        verify!(code.to_bytes(), b"[player_party_position] ulath 1 1\n");
    }

    #[test]
    fn code_62() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'6', b'2'), b"Killer orc 1 50 101 200 202 303 404 504 ekuva_ja_expaa 1 1 1 0 0 0 0 1 0 0 0 0 0 0 0 12345 100000 1234 Wed_Oct_31_15:57:52_2007");
        verify!(code.to_bytes(), b"[party_player_status] Killer orc 1 50 101 200 202 303 404 504 ekuva_ja_expaa 1 1 1 0 0 0 0 1 0 0 0 0 0 0 0 12345 100000 1234 Wed_Oct_31_15:57:52_2007\n");
    }

    #[test]
    fn code_63() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'6', b'3'), b"ulath");
        verify!(code.to_bytes(), b"[party_player_left] ulath\n");
    }

    #[test]
    fn code_64() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'6', b'4'), b"lay_on_hands 120");
        verify!(code.to_bytes(), b"[player_effect] lay_on_hands 120\n");
    }

    #[test]
    fn code_70() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'7', b'0'), b"evilmonster 45");
        verify!(code.to_bytes(), b"[player_target] evilmonster 45\n");
    }

    #[test]
    fn code_99() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'9', b'9'), b"1 dex 300");
        verify!(code.to_bytes(), b"[custom_info:0] 1 dex 300\n");
    }

    #[test]
    fn code_99_multiple_line() {
        let _ = env_logger::try_init();
        let code = mk_code!((b'9', b'9'), b"r1\nr2\nr3", b"custom_info");
        verify!(code.to_bytes(), b"[custom_info:0] r1\n[custom_info:1] r2\n[custom_info:2] r3\n");
    }
}

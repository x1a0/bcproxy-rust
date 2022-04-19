use bytes::{BytesMut, Bytes, BufMut};

use crate::color::{colorize_content, FgBg};

// 🦇
const BAT: [u8; 4] = [0xF0, 0x9F, 0xA6, 0x87];

const MESSAGE_OF_TYPE: (u8, u8) = (b'1', b'0');
const MESSAGE_TYPE_SPEC_PROMPT: &str = "spec_prompt";

const CLEAR_SCREEN: (u8, u8) = (b'1', b'1');

const TEXT_COLOR_FG: (u8, u8) = (b'2', b'0');
const TEXT_COLOR_BG: (u8, u8) = (b'2', b'1');
const TEXT_BOLD: (u8, u8) = (b'2', b'2');
const TEXT_ITALIC: (u8, u8) = (b'2', b'3');
const TEXT_UNDERLINE: (u8, u8) = (b'2', b'4');
const TEXT_BLINK: (u8, u8) = (b'2', b'5');
const TEXT_RESET: (u8, u8) = (b'2', b'9');

const TEXT_HYPERLINK: (u8, u8) = (b'3', b'0');
const TEXT_IN_GAME_LINK: (u8, u8) = (b'3', b'1');

const PLAYER_FULL_HP_SP_EP: (u8, u8) = (b'5', b'0');
const PLAYER_PARTIAL_HP_SP_EP: (u8, u8) = (b'5', b'1');
const PLAYER_INFO: (u8, u8) = (b'5', b'2');
const PLAYER_FREE_EXP: (u8, u8) = (b'5', b'3');
const PLAYER_STATUS: (u8, u8) = (b'5', b'4');
const PLAYER_LOCATION: (u8, u8) = (b'6', b'0');

const STATUS_AFFECTING: (u8, u8) = (b'6', b'4');

const PARTY_LOCATION: (u8, u8) = (b'6', b'1');
const PARTY_FULL_STATUS: (u8, u8) = (b'6', b'2');
const PARTY_PLAYER_LEFT: (u8, u8) = (b'6', b'3');

const TARGET_INFO: (u8, u8) = (b'7', b'0');

const CUSTOM_INFO: (u8, u8) = (b'9', b'9');


#[derive(Clone, Debug, PartialEq)]
pub struct BatTag {
    pub code: (u8, u8),
    pub argument: Option<Bytes>,
    pub content: BytesMut,
}

impl BatTag {
    pub fn new(code: (u8, u8)) -> BatTag {
        BatTag {
            code,
            argument: None,
            content: BytesMut::with_capacity(4096),
        }
    }
}

impl std::fmt::Display for BatTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "code: {}{}", self.code.0 as char, self.code.1 as char)
    }
}

impl From<BatTag> for BytesMut {
    fn from(mut tag: BatTag) -> Self {
        match tag.code {
            MESSAGE_OF_TYPE => {
                let mut use_bat_tag = true;

                // Remove uninterested argument
                if let Some(ref bytes) = tag.argument {
                    match std::str::from_utf8(bytes) {
                        Ok("spec_battle") |
                        Ok("spec_spell") |
                        Ok("spec_skill") => {
                            tag.argument = None;
                            use_bat_tag = false;
                        }
                        _ => {}
                    }
                }

                // If the content has multiple lines, prepend "🦇10" and argument
                // to each line.
                if !tag.content.ends_with(&[b'\n']) {
                    tag.content.put_u8(b'\n');
                }

                let lines = tag.content.iter().filter(|x| **x == b'\n').count();

                let arg_len = tag.argument.as_ref().map_or(0, |a| a.len());

                let mut bytes = BytesMut::with_capacity(
                    lines * (
                        if use_bat_tag { BAT.len() } else { 0 }
                        + if use_bat_tag { 2 } else { 0 } // tag code (10)
                        + arg_len
                        + 1 // space
                    )
                    + tag.content.len()
                );

                let argument = tag.argument.map_or(vec![], |bytes| bytes.to_vec());
                while let Some(offset) = tag.content.iter().position(|x| *x == b'\n') {
                    let line = tag.content.split_to(offset + 1);
                    if use_bat_tag {
                        bytes.extend(BAT);
                        bytes.extend([b'1', b'0']);
                        bytes.extend(argument.clone());
                        bytes.put_u8(b' ');
                    }
                    bytes.put(line);
                }

                bytes
            }
            CLEAR_SCREEN => {
                let mut bytes = BytesMut::with_capacity(
                    BAT.len()
                    + 2 // tag code (11)
                );
                bytes.extend(BAT);
                bytes.extend([b'1', b'1']);
                bytes
            }
            TEXT_COLOR_FG | TEXT_COLOR_BG => {
                let fg_bg = match tag.code.1 {
                    b'0' => FgBg::FG,
                    _ => FgBg::BG,
                };

                // color + content + reset = 11 + <content> + 4
                let mut bytes = BytesMut::with_capacity(11 + tag.content.len());
                bytes.put(colorize_content(&tag.content, tag.argument.unwrap().to_vec(), fg_bg));
                bytes
            }
            TEXT_BOLD |
            TEXT_ITALIC |
            TEXT_UNDERLINE |
            TEXT_BLINK |
            TEXT_RESET => {
                let code = match tag.code.1 {
                    b'2' => 1, // bold
                    b'3' => 3, // italic
                    b'4' => 4, // underline
                    b'5' => 5, // blink
                    _ => 0,    // reset
                };
                let mut bytes = BytesMut::with_capacity(4 + tag.content.len());
                bytes.extend([0x1b, b'[', code, b'm']);
                bytes.put(tag.content);
                bytes
            }
            TEXT_HYPERLINK |
            TEXT_IN_GAME_LINK => {
                // hyperlink and in-game link - no support yet, just output as normal text
                tag.content
            }
            CUSTOM_INFO => {
                // If the content has multiple lines, prepend "🦇99" to each line.
                if !tag.content.ends_with(&[b'\n']) {
                    tag.content.put_u8(b'\n');
                }

                let lines = tag.content.iter().filter(|x| **x == b'\n').count();

                let mut bytes = BytesMut::with_capacity(
                    lines * (
                        BAT.len()
                        + 2 // tag code (99)
                        + 1 // space
                    )
                    + tag.content.len()
                    + BAT.len() + 2 + BAT.len() + 1 // ending line
                );

                while let Some(offset) = tag.content.iter().position(|x| *x == b'\n') {
                    let line = tag.content.split_to(offset + 1);
                    bytes.extend(BAT);
                    bytes.extend([b'9', b'9']);
                    bytes.put_u8(b' ');
                    bytes.put(line);
                }

                // print ending line to tell client it's over
                bytes.extend(BAT);
                bytes.extend([b'9', b'9']);
                bytes.extend(BAT);
                bytes.put_u8(b'\n');

                bytes
            }
            (_, _) => {
                let mut bytes = BytesMut::with_capacity(
                    BAT.len()
                    + 3 // tag code + space
                    + tag.content.len()
                    + 1 // newline
                    + 2 // GA
                );

                bytes.extend(BAT);
                bytes.extend([tag.code.0, tag.code.1, b' ']);
                bytes.put(tag.content);
                bytes.extend([b'\n', 0xff, 0xf9]);
                bytes
            }
        }
    }
}

impl From<BatTag> for Bytes {
    fn from(tag: BatTag) -> Self {
        BytesMut::from(tag).freeze()
    }
}

use std::io;

use bytes::BytesMut;

use crate::color::color256::rgb_to_256;

/// 🦇
const BAT: [u8; 4] = [0xF0, 0x9F, 0xA6, 0x87];

const MESSAGE_OF_TYPE: (u8, u8) = (b'1', b'0');
const MESSAGE_TYPE_SPEC_PROMPT: &str = "spec_prompt";

const CLEAR_SCREEN: (u8, u8) = (b'1', b'1');

const TEXT_COLOR_FOREGROUND: (u8, u8) = (b'2', b'0');
const TEXT_COLOR_BACKGROUND: (u8, u8) = (b'2', b'1');
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

#[derive(Debug, PartialEq)]
pub struct ControlCode {
    code: (u8, u8),
    attribute: Vec<u8>,
    children: Vec<ControlCodeContent>,
}

#[derive(Debug, PartialEq)]
enum ControlCodeContent {
    Text(Vec<u8>),
    Code(ControlCode),
}

impl ControlCode {
    pub fn from(bytes: BytesMut) -> io::Result<Self> {
        let code = (bytes[2], bytes[3]);
        let mut attribute: Vec<u8> = Vec::new();
        let mut children: Vec<ControlCodeContent> = Vec::new();
        let mut depth = 0;

        // skip the first 4 bytes which must be \x1b<XX
        let mut index = 4;

        loop {
            let next_esc_index =
                bytes[index..]
                    .iter()
                    .position(|b| *b == 0x1b)
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "unterminated control code")
                    })?
                    + index;

            match (bytes[next_esc_index + 1], depth) {
                (b'>', 0) => {
                    // no more to parse, create the control code
                    if index < next_esc_index {
                        children.push(ControlCodeContent::Text(
                            bytes[index..next_esc_index].to_vec(),
                        ));
                    }

                    return Ok(Self {
                        code,
                        attribute,
                        children,
                    });
                }
                (b'>', _) => {
                    depth -= 1;
                    index = next_esc_index + 4;
                    continue;
                }
                (b'|', _) => {
                    attribute = bytes[index..next_esc_index].to_vec();
                    index = next_esc_index + 2;
                    continue;
                }
                (b'<', _) => {
                    if index < next_esc_index {
                        children.push(ControlCodeContent::Text(
                            bytes[index..next_esc_index].to_vec(),
                        ));
                    }

                    depth += 1;
                    let mut inner_index = next_esc_index + 4;

                    loop {
                        let next_inner_esc_index = bytes[inner_index..]
                            .iter()
                            .position(|b| *b == 0x1b)
                            .ok_or_else(|| {
                                io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    "unterminated inner control code",
                                )
                            })?
                            + inner_index;

                        match bytes[next_inner_esc_index + 1] {
                            b'|' => {
                                inner_index = next_inner_esc_index + 2;
                                continue;
                            }
                            b'<' => {
                                depth += 1;
                                inner_index = next_inner_esc_index + 4;
                                continue;
                            }
                            b'>' => {
                                depth -= 1;

                                if depth > 0 {
                                    inner_index = next_inner_esc_index + 4;
                                    continue;
                                }

                                let inner_bytes = &bytes[next_esc_index..next_inner_esc_index + 4];
                                let child_code = ControlCode::from(inner_bytes.into())?;

                                children.push(ControlCodeContent::Code(child_code));

                                // set the main index to be after the inner code
                                index = next_inner_esc_index + 4;

                                break;
                            }
                            _ => {
                                inner_index = next_inner_esc_index + 1;
                            }
                        }
                    }
                }
                (_, _) => {
                    index = next_esc_index + 1;
                }
            }
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let children_bytes: Vec<u8> = self
            .children
            .iter()
            .flat_map(|c| match c {
                ControlCodeContent::Text(bytes) => bytes.clone(),
                ControlCodeContent::Code(c) => c.to_bytes(),
            })
            .collect();

        match self.code {
            TEXT_COLOR_FOREGROUND => {
                let mut bytes = Vec::with_capacity(children_bytes.len() + 15);
                let color = rgb_to_256(&self.attribute);
                bytes.extend_from_slice(format!("\x1b[38;5;{color}m").as_bytes());
                bytes.extend(children_bytes);
                bytes.extend_from_slice(b"\x1b[0m");
                bytes
            }

            TEXT_COLOR_BACKGROUND => {
                let mut bytes = Vec::with_capacity(children_bytes.len() + 15);
                let color = rgb_to_256(&self.attribute);
                bytes.extend_from_slice(format!("\x1b[48;5;{color}m").as_bytes());
                bytes.extend(children_bytes);
                bytes.extend_from_slice(b"\x1b[0m");
                bytes
            }

            TEXT_BOLD => {
                let mut bytes = Vec::with_capacity(children_bytes.len() + 8);
                bytes.extend_from_slice("\x1b[1m".to_string().as_bytes());
                bytes.extend(children_bytes);
                bytes.extend_from_slice(b"\x1b[0m");
                bytes
            }

            TEXT_ITALIC => {
                let mut bytes = Vec::with_capacity(children_bytes.len() + 8);
                bytes.extend_from_slice("\x1b[3m".to_string().as_bytes());
                bytes.extend(children_bytes);
                bytes.extend_from_slice(b"\x1b[0m");
                bytes
            }

            TEXT_UNDERLINE => {
                let mut bytes = Vec::with_capacity(children_bytes.len() + 8);
                bytes.extend_from_slice("\x1b[4m".to_string().as_bytes());
                bytes.extend(children_bytes);
                bytes.extend_from_slice(b"\x1b[0m");
                bytes
            }

            TEXT_BLINK => {
                let mut bytes = Vec::with_capacity(children_bytes.len() + 8);
                bytes.extend_from_slice("\x1b[5m".to_string().as_bytes());
                bytes.extend(children_bytes);
                bytes.extend_from_slice(b"\x1b[0m");
                bytes
            }

            TEXT_RESET => {
                let mut bytes = Vec::with_capacity(children_bytes.len() + 8);
                bytes.extend_from_slice("\x1b[0m".to_string().as_bytes());
                bytes.extend(children_bytes);
                bytes
            }

            (_, _) => children_bytes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stacked_code_from_bytes() {
        let bytes: BytesMut =
            "\x1b<10map\x1b|top level value\x1b<20FF0000\x1b|This is a test\x1b>20some more top level value\x1b<2000FF00\x1b|this is another test\x1b>20\x1b>10".into();
        let code = ControlCode::from(bytes).unwrap();

        assert_eq!(
            code,
            ControlCode {
                code: (b'1', b'0'),
                attribute: b"map".to_vec(),
                children: vec![
                    ControlCodeContent::Text(b"top level value".to_vec()),
                    ControlCodeContent::Code(ControlCode {
                        code: (b'2', b'0'),
                        attribute: b"FF0000".to_vec(),
                        children: vec![ControlCodeContent::Text(b"This is a test".to_vec())]
                    }),
                    ControlCodeContent::Text(b"some more top level value".to_vec()),
                    ControlCodeContent::Code(ControlCode {
                        code: (b'2', b'0'),
                        attribute: b"00FF00".to_vec(),
                        children: vec![ControlCodeContent::Text(b"this is another test".to_vec())]
                    })
                ]
            }
        );
    }

    #[test]
    fn deep_stacked_code_from_bytes() {
        let bytes: BytesMut =
            "\x1b<10map\x1b|\x1b<20FFFFFF\x1b|\x1b<210000FF\x1b|Test output, white on blue\x1b>21\x1b>20\x1b>10".into();
        let code = ControlCode::from(bytes).unwrap();

        assert_eq!(
            code,
            ControlCode {
                code: (b'1', b'0'),
                attribute: b"map".to_vec(),
                children: vec![ControlCodeContent::Code(ControlCode {
                    code: (b'2', b'0'),
                    attribute: b"FFFFFF".to_vec(),
                    children: vec![ControlCodeContent::Code(ControlCode {
                        code: (b'2', b'1'),
                        attribute: b"0000FF".to_vec(),
                        children: vec![ControlCodeContent::Text(
                            b"Test output, white on blue".to_vec()
                        )]
                    })]
                })]
            }
        );
    }

    #[test]
    fn code_with_newline_in_value_from_bytes() {
        let bytes: BytesMut =
            "\x1b<10map\x1b|\x1b<20FFFFFF\x1b|\x1b<210000FF\x1b|Test output, white on blue\nAnother line\x1b>21\x1b>20\x1b>10".into();
        let code = ControlCode::from(bytes).unwrap();

        assert_eq!(
            code,
            ControlCode {
                code: (b'1', b'0'),
                attribute: b"map".to_vec(),
                children: vec![ControlCodeContent::Code(ControlCode {
                    code: (b'2', b'0'),
                    attribute: b"FFFFFF".to_vec(),
                    children: vec![ControlCodeContent::Code(ControlCode {
                        code: (b'2', b'1'),
                        attribute: b"0000FF".to_vec(),
                        children: vec![ControlCodeContent::Text(
                            b"Test output, white on blue\nAnother line".to_vec()
                        )]
                    })]
                })]
            }
        );
    }
}

use super::ControlCode;
use super::ControlCodeContent;
use crate::color::color256::rgb_to_256;

const IAC_GA: [u8; 2] = [0xff, 0xf9];

const PREFIX: [u8; 2] = [0xcf, 0x80];

const MESSAGE_OF_TYPE: (u8, u8) = (b'1', b'0');

// spec_prompt
const MESSAGE_TYPE_SPEC_PROMPT: [u8; 11] = [
    b's', b'p', b'e', b'c', b'_', b'p', b'r', b'o', b'm', b'p', b't',
];
// spec_skill
const MESSAGE_TYPE_SPEC_SKILL: [u8; 10] =
    [b's', b'p', b'e', b'c', b'_', b's', b'k', b'i', b'l', b'l'];
// spec_spell
const MESSAGE_TYPE_SPEC_SPELL: [u8; 10] =
    [b's', b'p', b'e', b'c', b'_', b's', b'p', b'e', b'l', b'l'];

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

macro_rules! embed_style {
    ($style:expr, $children_bytes:expr) => {{
        let mut bytes = Vec::with_capacity($children_bytes.len() + 8);
        bytes.extend_from_slice("\x1b[$stylem".to_string().as_bytes());
        bytes.extend($children_bytes);
        bytes.extend_from_slice(b"\x1b[0m");
        bytes
    }};
}

macro_rules! embed_info_type {
    ($c1:expr, $c2:expr, $children_bytes:expr) => {{
        let mut bytes = Vec::with_capacity($children_bytes.len() + PREFIX.len() + 20);
        bytes.extend(PREFIX);
        bytes.push($c1);
        bytes.push($c2);
        bytes.push(b':');
        bytes.extend($children_bytes);
        bytes.push(b'\n');
        bytes
    }};
}

impl ControlCode {
    pub fn get_children_bytes(&self) -> Vec<u8> {
        self.children
            .iter()
            .flat_map(|c| match c {
                ControlCodeContent::Text(bytes) => bytes.clone(),
                ControlCodeContent::Code(c) => c.to_bytes(),
            })
            .collect()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let children_bytes = self.get_children_bytes();

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
                embed_style!(1, children_bytes)
            }

            TEXT_ITALIC => {
                embed_style!(3, children_bytes)
            }

            TEXT_UNDERLINE => {
                embed_style!(4, children_bytes)
            }

            TEXT_BLINK => {
                embed_style!(5, children_bytes)
            }

            TEXT_RESET => {
                let mut bytes = Vec::with_capacity(children_bytes.len() + 8);
                bytes.extend_from_slice("\x1b[0m".to_string().as_bytes());
                bytes.extend(children_bytes);
                bytes
            }

            MESSAGE_OF_TYPE if self.attribute == MESSAGE_TYPE_SPEC_PROMPT => {
                let mut bytes =
                    Vec::with_capacity(PREFIX.len() + 1 + children_bytes.len() + IAC_GA.len());
                bytes.extend(PREFIX);
                bytes.push(b'>');
                bytes.extend(children_bytes);
                bytes.extend(IAC_GA);
                bytes
            }

            MESSAGE_OF_TYPE
                if self.attribute == MESSAGE_TYPE_SPEC_SKILL
                    || self.attribute == MESSAGE_TYPE_SPEC_SPELL =>
            {
                children_bytes
            }

            MESSAGE_OF_TYPE => {
                let lines = children_bytes.split(|b| *b == b'\n').collect::<Vec<_>>();
                let mut bytes = Vec::with_capacity(
                    (PREFIX.len() + self.attribute.len() + 2) * lines.len() + children_bytes.len(),
                );
                for line in &lines[0..lines.len() - 1] {
                    bytes.extend(PREFIX);
                    bytes.extend(&self.attribute);
                    bytes.extend(b": ");
                    bytes.extend(*line);
                    bytes.push(b'\n');
                }
                bytes
            }

            CLEAR_SCREEN => b"<clear>\n".to_vec(),

            PLAYER_FULL_HP_SP_EP
            | PLAYER_PARTIAL_HP_SP_EP
            | PLAYER_INFO
            | PLAYER_FREE_EXP
            | PLAYER_STATUS
            | PLAYER_LOCATION
            | STATUS_AFFECTING
            | PARTY_LOCATION
            | PARTY_FULL_STATUS
            | PARTY_PLAYER_LEFT
            | TARGET_INFO => {
                embed_info_type!(self.code.0, self.code.1, children_bytes)
            }

            CUSTOM_INFO => {
                // \x1b<99BAT_MAPPER;;friday (player city);;$apr1$dF!!_X#W$ENKn9g1ViErH4mjKkn4qR/;;west;;0;;(road) Friday;;\x1b[32mc\x1b[0m\x1b[32mc\x1b[0m\x1b[32mc\x1b[0m\x1b[32mc\x1b[0m\x1b[37m%\x1b[0m+     \r\n\x1b[37m.\x1b[0m\x1b[37m.\x1b[0m\x1b[37m.\x1b[0m\x1b[37m#\x1b[0m\x1b[36mI\x1b[0m+   \r\n\x1b[37m.\x1b[0m\x1b[37m.\x1b[0m\x1b[37m.\x1b[0m\x1b[31mG\x1b[0m*+   \r\n\x1b[37m.\x1b[0m\x1b[37m.\x1b[0m\x1b[37m.\x1b[0m\x1b[37m#\x1b[0m\x1b[1;33mH\x1b[0m+   \r\n\x1b[32mc\x1b[0m\x1b[32mc\x1b[0m\x1b[32mc\x1b[0m\x1b[32mc\x1b[0m\x1b[1;33mH\x1b[0m+   \r\n;;north,northeast,east,southeast,south,west;;\x1b>99\x1b
                let mut fields: Vec<&[u8]> = Vec::with_capacity(8);
                let mut next_index = 0;
                let mut read_index = 0;

                while let Some(index) = children_bytes[next_index..].iter().position(|b| *b == b';')
                {
                    if next_index + index < children_bytes.len() - 1
                        && children_bytes[next_index + index + 1] == b';'
                    {
                        fields.push(&children_bytes[read_index..next_index + index]);
                        next_index += index + 2;
                        read_index = next_index;
                    } else {
                        next_index += index + 1;
                    }
                }

                if next_index < children_bytes.len() {
                    fields.push(&children_bytes[next_index..]);
                }

                match fields.len() {
                    8 => {
                        let lines = fields[6].split(|b| *b == b'\n').collect::<Vec<_>>();
                        let mut bytes = Vec::with_capacity(
                            (PREFIX.len() + 4) * lines.len() + children_bytes.len(),
                        );
                        bytes.extend(PREFIX);
                        bytes.push(self.code.0);
                        bytes.push(self.code.1);
                        bytes.extend(b":");
                        fields.iter().take(6).for_each(|f| {
                            bytes.extend(&f[..]);
                            bytes.extend(b";;");
                        });
                        bytes.extend(fields[7]);
                        bytes.extend(b";;");
                        bytes.push(b'\n');

                        for line in &lines[0..std::cmp::max(lines.len() - 1, 1)] {
                            bytes.extend(PREFIX);
                            bytes.push(self.code.0);
                            bytes.push(self.code.1);
                            bytes.extend(b":");
                            bytes.extend(*line);
                            bytes.push(b'\n');
                        }

                        bytes
                    }

                    2 => {
                        let mut bytes = Vec::with_capacity(PREFIX.len() + 4 + children_bytes.len());
                        bytes.extend(PREFIX);
                        bytes.push(self.code.0);
                        bytes.push(self.code.1);
                        bytes.extend(b":");
                        bytes.extend(fields[1]);
                        bytes.push(b'\n');

                        bytes
                    }
                    _ => {
                        tracing::error!("cannot transform mapper with fields: {:?}", fields);
                        vec![]
                    }
                }
            }

            TEXT_HYPERLINK | TEXT_IN_GAME_LINK => children_bytes,

            (_, _) => embed_info_type!(self.code.0, self.code.1, children_bytes),
        }
    }

    pub fn is_mapper(&self) -> bool {
        self.code == CUSTOM_INFO
    }
}

#[cfg(test)]
mod tests {
    use bytes::BytesMut;

    use super::*;

    #[test]
    fn test_transform_mapper_area() {
        let bytes = BytesMut::from(
            &b"\x1b<99BAT_MAPPER;;area;;id;;from;;0;;short;;long;;exits;;\x1b>99"[..],
        );
        let control_code = ControlCode::from(bytes).unwrap();

        let mut expected: Vec<u8> = Vec::new();
        expected.extend(PREFIX);
        expected.extend(b"99:BAT_MAPPER;;area;;id;;from;;0;;short;;exits;;\n");
        expected.extend(PREFIX);
        expected.extend(b"99:long\n");

        assert_eq!(control_code.to_bytes(), expected);
    }

    #[test]
    fn test_transform_mapper_realm() {
        let bytes = BytesMut::from(&b"\x1b<99BAT_MAPPER;;REALM_MAP\x1b>99"[..]);
        let control_code = ControlCode::from(bytes).unwrap();

        let mut expected: Vec<u8> = Vec::new();
        expected.extend(PREFIX);
        expected.extend(b"99:BAT_MAPPER;;REALM_MAP\n");

        assert_eq!(control_code.to_bytes(), expected);
    }

    #[test]
    fn test_friday() {
        let bytes = BytesMut::from(&b"\x1b<99BAT_MAPPER;;friday (player city);;$apr1$dF!!_X#W$ENKn9g1ViErH4mjKkn4qR/;;north;;0;;(road) Friday;;\x1b[32mc\x1b[0m\x1b[32mc\x1b[0m\x1b[32mc\x1b[0m\x1b[32mc\x1b[0m\x1b[37m%\x1b[0m+     \r\n\x1b[37m.\x1b[0m\x1b[37m.\x1b[0m\x1b[37m.\x1b[0m\x1b[37m#\x1b[0m\x1b[36mI\x1b[0m+   \r\n\x1b[37m.\x1b[0m\x1b[37m.\x1b[0m\x1b[37m.\x1b[0m\x1b[31mG\x1b[0m*+   \r\n\x1b[37m.\x1b[0m\x1b[37m.\x1b[0m\x1b[37m.\x1b[0m\x1b[37m#\x1b[0m\x1b[1;33mH\x1b[0m+   \r\n\x1b[32mc\x1b[0m\x1b[32mc\x1b[0m\x1b[32mc\x1b[0m\x1b[32mc\x1b[0m\x1b[1;33mH\x1b[0m+   \r\n;;north,northeast,east,southeast,south,west;;\x1b>99"[..]);
        let control_code = ControlCode::from(bytes).unwrap();

        let mut expected = BytesMut::new();
        expected.extend(PREFIX);
        expected.extend(b"99:BAT_MAPPER;;friday (player city);;$apr1$dF!!_X#W$ENKn9g1ViErH4mjKkn4qR/;;north;;0;;(road) Friday;;north,northeast,east,southeast,south,west;;\n");
        expected.extend(PREFIX);
        expected.extend(b"99:\x1b[32mc\x1b[0m\x1b[32mc\x1b[0m\x1b[32mc\x1b[0m\x1b[32mc\x1b[0m\x1b[37m%\x1b[0m+     \r\n");
        expected.extend(PREFIX);
        expected.extend(b"99:\x1b[37m.\x1b[0m\x1b[37m.\x1b[0m\x1b[37m.\x1b[0m\x1b[37m#\x1b[0m\x1b[36mI\x1b[0m+   \r\n");
        expected.extend(PREFIX);
        expected.extend(
            b"99:\x1b[37m.\x1b[0m\x1b[37m.\x1b[0m\x1b[37m.\x1b[0m\x1b[31mG\x1b[0m*+   \r\n",
        );
        expected.extend(PREFIX);
        expected.extend(b"99:\x1b[37m.\x1b[0m\x1b[37m.\x1b[0m\x1b[37m.\x1b[0m\x1b[37m#\x1b[0m\x1b[1;33mH\x1b[0m+   \r\n");
        expected.extend(PREFIX);
        expected.extend(b"99:\x1b[32mc\x1b[0m\x1b[32mc\x1b[0m\x1b[32mc\x1b[0m\x1b[32mc\x1b[0m\x1b[1;33mH\x1b[0m+   \r\n");
        assert_eq!(BytesMut::from(&control_code.to_bytes()[..]), expected);
    }
}

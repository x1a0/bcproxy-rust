use std::io;

use bytes::BytesMut;

#[derive(Debug, PartialEq)]
pub struct ControlCode {
    pub code: (u8, u8),
    pub attribute: Vec<u8>,
    pub value: Vec<u8>,
    pub children: Vec<ControlCode>,
}

impl ControlCode {
    pub fn from(bytes: BytesMut) -> io::Result<Self> {
        let code = (bytes[1], bytes[2]);
        let mut index = 4;
        let mut attribute: Vec<u8> = Vec::new();
        let mut value: Vec<u8> = Vec::new();
        let mut children: Vec<ControlCode> = Vec::new();
        let mut depth = 0;

        loop {
            let next_esc_index =
                bytes[index..]
                    .iter()
                    .position(|b| *b == 0x1b)
                    .ok_or(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "unterminated control code",
                    ))?
                    + index;

            // no more to parse, create the control code
            if bytes[next_esc_index + 1] == b'>' {
                if depth == 0 {
                    value.append(bytes[index..next_esc_index].to_vec().as_mut());

                    return Ok(Self {
                        code,
                        attribute,
                        value,
                        children,
                    });
                } else {
                    depth -= 1;
                    index = next_esc_index + 4;
                    continue;
                }
            }

            if bytes[next_esc_index + 1] == b'|' {
                attribute = bytes[index..next_esc_index].to_vec();
                index = next_esc_index + 2;
                continue;
            }

            if bytes[next_esc_index + 1] == b'<' {
                println!("index: {}", index);
                println!("next_esc_index: {}", next_esc_index);
                if index < next_esc_index {
                    value.append(bytes[index..next_esc_index].to_vec().as_mut());
                }

                depth += 1;
                let mut inner_index = next_esc_index + 2;

                loop {
                    let next_inner_esc_index =
                        bytes[inner_index..].iter().position(|b| *b == 0x1b).ok_or(
                            io::Error::new(io::ErrorKind::InvalidData, "unterminated control code"),
                        )? + inner_index;

                    if bytes[next_inner_esc_index + 1] == b'|' {
                        inner_index += 2;
                        continue;
                    }

                    if bytes[next_inner_esc_index + 1] == b'<' {
                        depth += 1;
                        continue;
                    }

                    if bytes[next_inner_esc_index + 1] == b'>' {
                        depth -= 1;

                        if depth > 0 {
                            continue;
                        }

                        let inner_bytes = &bytes[next_esc_index..next_inner_esc_index + 4];
                        let child = ControlCode::from(inner_bytes.into())?;
                        children.push(child);

                        // set the main index to be after the inner code
                        index = next_inner_esc_index + 4;

                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_from_bytes() {
        let bytes = BytesMut::from(
            &b"\x1b<10map\x1b|top level value\x1b<20FF0000\x1b|This is a test\x1b>20\x1b>10"[..],
        );
        let code = ControlCode::from(bytes).unwrap();
        let v = &code.value;
        println!("{:?}", String::from_utf8(v.to_vec()));
    }
}

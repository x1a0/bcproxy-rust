use bytes::Bytes;

#[derive(Debug, PartialEq)]
pub(crate) enum Mapper {
    Area {
        room_id: String,
        room_name: String,
        area_name: String,
        room_description: String,
        indoor: bool,
        exits: String,
        from: String,
    },
    Realm,
}

impl TryFrom<&[u8]> for Mapper {
    type Error = std::io::Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let mut field_index = 0;

        let mut room_id = String::new();
        let mut room_name = String::new();
        let mut area_name = String::new();
        let mut room_description = String::new();
        let mut indoor = false;
        let mut from = String::new();
        let exits: String;

        let mut next_index = 0;
        loop {
            match (
                field_index,
                value[next_index..].windows(2).position(|b| b == b";;"),
            ) {
                (0, Some(index)) => {
                    // BAT_MAPPER
                    next_index += index + 2;
                    field_index += 1;
                }
                (1, Some(index)) => {
                    area_name = String::from_utf8(value[next_index..next_index + index].to_vec())
                        .map_err(map_ut8_error)?;
                    next_index += index + 2;
                    field_index += 1;
                }
                (1, None) => {
                    return Ok(Self::Realm);
                }
                (2, Some(index)) => {
                    room_id = String::from_utf8(value[next_index..next_index + index].to_vec())
                        .map_err(map_ut8_error)?;
                    next_index += index + 2;
                    field_index += 1;
                }
                (3, Some(index)) => {
                    from = String::from_utf8(value[next_index..next_index + index].to_vec())
                        .map_err(map_ut8_error)?;
                    next_index += index + 2;
                    field_index += 1;
                }
                (4, Some(index)) => {
                    indoor = value[next_index] == b'1';
                    next_index += index + 2;
                    field_index += 1;
                }
                (5, Some(index)) => {
                    room_name = String::from_utf8(value[next_index..next_index + index].to_vec())
                        .map_err(map_ut8_error)?;
                    next_index += index + 2;
                    field_index += 1;
                }
                (6, Some(index)) => {
                    room_description =
                        String::from_utf8(value[next_index..next_index + index].to_vec())
                            .map_err(map_ut8_error)?;
                    next_index += index + 2;
                    field_index += 1;
                }
                (7, Some(index)) => {
                    exits = String::from_utf8(value[next_index..next_index + index].to_vec())
                        .map_err(map_ut8_error)?;
                    break;
                }

                _ => {
                    tracing::debug!("{:?}", Bytes::from(value.to_vec()));
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "cannot get correct room data from input",
                    ));
                }
            }
        }

        Ok(Self::Area {
            room_id,
            room_name,
            area_name,
            room_description,
            indoor,
            exits,
            from,
        })
    }
}

fn map_ut8_error(e: std::string::FromUtf8Error) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_from_area() {
        let input =
            &b"BAT_MAPPER;;area;;room_id;;from;;1;;room_short;;room_long\nanother line;;exits;;"[..];
        let mapper = Mapper::try_from(input).unwrap();

        assert_eq!(
            mapper,
            Mapper::Area {
                room_id: "room_id".to_string(),
                room_name: "room_short".to_string(),
                area_name: "area".to_string(),
                room_description: "room_long\nanother line".to_string(),
                indoor: true,
                exits: "exits".to_string(),
                from: "from".to_string(),
            }
        );
    }

    #[test]
    fn test_try_from_realm() {
        let input = &b"BAT_MAPPER;;REALM_MAP"[..];
        let mapper = Mapper::try_from(input).unwrap();

        assert_eq!(mapper, Mapper::Realm);
    }
}

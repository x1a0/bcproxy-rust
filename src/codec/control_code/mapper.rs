use bytes::Bytes;

pub(crate) struct Mapper {
    pub room_id: String,
    pub room_name: String,
    pub area_name: String,
    pub room_description: String,
    pub indoor: bool,
    pub exits: String,
    pub from: String,
}

impl TryFrom<&[u8]> for Mapper {
    type Error = std::io::Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        // BAT_MAPPER;;area;;id;;from_dir;;indoor;;room_short;;room_long;;exits;;
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

        Ok(Self {
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
    fn test_mapper_try_from() {
        let input =
            &b"BAT_MAPPER;;area;;room_id;;from;;1;;room_short;;room_long\nanother line;;exits;;"[..];
        let mapper = Mapper::try_from(input).unwrap();

        assert_eq!(mapper.area_name, "area");
        assert_eq!(mapper.room_id, "room_id");
        assert_eq!(mapper.from, "from");
        assert!(mapper.indoor);
        assert_eq!(mapper.room_name, "room_short");
        assert_eq!(mapper.room_description, "room_long\nanother line");
        assert_eq!(mapper.exits, "exits");
    }
}

use bytes::BytesMut;

use super::usize_to_chars;

#[derive(Clone, Debug)]
pub struct BatMapper {
    pub area: Option<String>,
    pub id: Option<String>,
    pub short_desc: Option<String>,
    pub long_desc: Option<String>,
    pub exits: Option<String>,
    pub is_indoor: Option<bool>,
    pub from_dir: Option<String>,
    pub from_room_id: Option<String>,
    pub output: BytesMut,
}

#[derive(Debug)]
enum ParseState {
    Area,
    RoomId,
    FromDirection,
    Indoor,
    ShortDesc,
    LongDesc,
    Exits,
}

fn latin1_to_string(bytes: &BytesMut) -> String {
    bytes.iter().map(|&c| c as char).collect()
}

impl BatMapper {
    pub fn new(mut input: BytesMut, from_room: Option<Box<BatMapper>>) -> BatMapper {
        let mut state = ParseState::Area;
        let mut cnt_long_desc_line = 0;

        let mut area = None;
        let mut id = None;
        let mut short_desc = None;
        let mut long_desc = None;
        let mut exits = None;
        let mut is_indoor = None;
        let mut from_dir = None;
        let mut output = BytesMut::with_capacity(input.len());

        while input.len() > 0 {
            let mut i = 0;
            for &b in input.as_ref() {
                if b == b';' || b == b'\n' {
                    break;
                } else {
                    i += 1;
                }
            }

            if i == input.len() {
                output.extend(b"[bat_mapper] ");
                output.extend(input);
                break;
            }

            match input[i] {
                b';' if input.len() - 1 > i && input[i + 1] == b';' => {

                    let mut bytes = input.split_to(i);

                    match state {
                        ParseState::Area => {
                            area = Some(latin1_to_string(&bytes));
                            output.extend(b"[bat_mapper:area] ");
                            state = ParseState::RoomId;
                        },

                        ParseState::RoomId => {
                            id = Some(latin1_to_string(&bytes));
                            output.extend(&[b'\n'][..]);
                            output.extend(b"[bat_mapper:id] ");
                            state = ParseState::FromDirection;
                        },

                        ParseState::FromDirection => {
                            from_dir = Some(latin1_to_string(&bytes));
                            output.extend(&[b'\n'][..]);
                            output.extend(b"[bat_mapper:from] ");
                            state = ParseState::Indoor;
                        },

                        ParseState::Indoor => {
                            if bytes.len() > 1 {
                                warn!("Bat mapper 'indoor' has more than 1 byte");
                            } else if bytes[0] == b'0' {
                                is_indoor = Some(false)
                            } else {
                                is_indoor = Some(true)
                            }

                            output.extend(&[b'\n'][..]);
                            output.extend(b"[bat_mapper:indoor] ");
                            state = ParseState::ShortDesc;
                        },

                        ParseState::ShortDesc => {
                            short_desc = Some(latin1_to_string(&bytes));
                            output.extend(&[b'\n'][..]);
                            output.extend(b"[bat_mapper:short] ");
                            state = ParseState::LongDesc;
                        },

                        ParseState::LongDesc => {
                            let s = latin1_to_string(&bytes);
                            if long_desc.is_none() {
                                long_desc = Some(s);
                            } else {
                                long_desc.as_mut().unwrap().push_str(&s);
                            }
                            state = ParseState::Exits;
                        },

                        ParseState::Exits => {
                            exits = Some(latin1_to_string(&bytes));
                            output.extend(b"[bat_mapper:exits] ");
                            state = ParseState::Exits;
                        },
                    }

                    if bytes.len() > 0 {
                        output.extend(bytes);
                    }

                    input.advance(2);
                },

                b';' => {
                    let bytes = input.split_to(i + 1);
                    output.extend(bytes);
                },

                b'\n' => {
                    let bytes = input.split_to(i + 1);

                    match state {
                        ParseState::LongDesc => {
                            if cnt_long_desc_line == 0 {
                                output.extend(&[b'\n']);
                            }

                            output.extend(b"[bat_mapper:long:");
                            output.extend(usize_to_chars(cnt_long_desc_line));
                            output.extend(b"] ");
                            cnt_long_desc_line += 1;

                            let s = latin1_to_string(&bytes);
                            if long_desc.is_none() {
                                long_desc = Some(s);
                            } else {
                                long_desc.as_mut().unwrap().push_str(&s);
                            }
                        },

                        _ => ()
                    }

                    output.extend(bytes);
                },

                _ => ()
            }
        }

        output.extend(&[b'\n'][..]);

        BatMapper {
            area: area,
            id: id,
            short_desc: short_desc,
            long_desc: long_desc,
            exits: exits,
            is_indoor: is_indoor,
            from_dir: from_dir,
            from_room_id: from_room.map(|x| x.id.unwrap()),
            output: output,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use env_logger;

    #[test]
    fn area_room() {
        let _ = env_logger::try_init();
        let bat_mapper = BatMapper::new(BytesMut::from(&b"arelium;;$apr1$dF!!_X#W$0QcXnT/1XhTQG7dSUp6WI.;;east;;1;;A Emergency Operations;;You stand in the middle of Emergency Operations.\nThe room is huge but silent. All the activity has ceased,\nor is there something wrong with you. There are several tables\nfull of equipment and few monitors showing something.\nThis is the place where we try to revive people who have lost\ntheir heartbeat, if you think there is something wrong with you\njust ask for help ('ask help', 'help me')\n;;west;;"[..]), None);

        assert_eq!(bat_mapper.output, &b"[bat_mapper:area] arelium\n[bat_mapper:id] $apr1$dF!!_X#W$0QcXnT/1XhTQG7dSUp6WI.\n[bat_mapper:from] east\n[bat_mapper:indoor] 1\n[bat_mapper:short] A Emergency Operations\n[bat_mapper:long:0] You stand in the middle of Emergency Operations.\n[bat_mapper:long:1] The room is huge but silent. All the activity has ceased,\n[bat_mapper:long:2] or is there something wrong with you. There are several tables\n[bat_mapper:long:3] full of equipment and few monitors showing something.\n[bat_mapper:long:4] This is the place where we try to revive people who have lost\n[bat_mapper:long:5] their heartbeat, if you think there is something wrong with you\n[bat_mapper:long:6] just ask for help ('ask help', 'help me')\n[bat_mapper:exits] west\n"[..]);

        assert_eq!(bat_mapper.area, Some(String::from("arelium")));
        assert_eq!(bat_mapper.id, Some(String::from("$apr1$dF!!_X#W$0QcXnT/1XhTQG7dSUp6WI.")));
        assert_eq!(bat_mapper.short_desc, Some(String::from("A Emergency Operations")));
        assert_eq!(bat_mapper.long_desc, Some(String::from("You stand in the middle of Emergency Operations.\nThe room is huge but silent. All the activity has ceased,\nor is there something wrong with you. There are several tables\nfull of equipment and few monitors showing something.\nThis is the place where we try to revive people who have lost\ntheir heartbeat, if you think there is something wrong with you\njust ask for help ('ask help', 'help me')\n")));
        assert_eq!(bat_mapper.exits, Some(String::from("west")));
        assert_eq!(bat_mapper.is_indoor, Some(true));
        assert_eq!(bat_mapper.from_dir, Some(String::from("east")));
    }

    #[test]
    fn realm_map() {
        let _ = env_logger::try_init();
        let bat_mapper = BatMapper::new(BytesMut::from(&b"REALM_MAP"[..]), None);
        assert_eq!(bat_mapper.output, &b"[bat_mapper] REALM_MAP\n"[..]);

        assert_eq!(bat_mapper.area, None);
        assert_eq!(bat_mapper.id, None);
        assert_eq!(bat_mapper.short_desc, None);
        assert_eq!(bat_mapper.long_desc, None);
        assert_eq!(bat_mapper.exits, None);
        assert_eq!(bat_mapper.is_indoor, None);
        assert_eq!(bat_mapper.from_dir, None);
    }
}

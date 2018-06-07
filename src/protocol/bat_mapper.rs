use bytes::BytesMut;

use super::*;

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
    pub monsters: Vec<Monster>,
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

impl BatMapper {
    pub fn new(mut input: BytesMut, monsters: Vec<Monster>, from_room: Option<Box<BatMapper>>) -> BatMapper {
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

        let mut next_index: usize = 0;

        while let Some(i) = input[next_index..].iter().position(|&b| b == b';' || b == b'\n') {
            let len = input.len();
            match input[next_index + i] {
                b';' if len > i + 1 && input[next_index + i + 1] == b';' => {

                    let mut bytes = input.split_to(i + next_index);

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
                    next_index = 0;
                },

                b';' => {
                    next_index = next_index + i + 1;
                },

                b'\n' => {
                    let bytes = input.split_to(next_index + i + 1);

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
                    next_index = 0;
                },

                _ => {
                    panic!("0 == 1");
                },
            }
        }

        match state {
            ParseState::Area => output.extend(&b"[bat_mapper] "[..]),
            _ => ()
        }

        if input.len() > 0 {
            output.extend(input);
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
            monsters: monsters,
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
        let bat_mapper = BatMapper::new(BytesMut::from(&b"\
dortlewall;;$apr1$dF!!_X#W$FQb5R4zU.u6aIWJRXqFlq1;;south;;1;;Monastery of Aeo;;\
You are just inside the monastery of Aeo in Dortlewall. The old monastery is\n\
to \x1b[38;5;10m\x1b[1msouth\x1b[0m\x1b[0m, and that is where you should head if you are here for the\n\
tutorial. To west you can see a small public library, and to east there is the\n\
new monastery, dug into the mountainside.\n\
;;south,west,east;;"[..]), None);

        assert_eq!(bat_mapper.output, BytesMut::from(&b"\
[bat_mapper:area] dortlewall\n\
[bat_mapper:id] $apr1$dF!!_X#W$FQb5R4zU.u6aIWJRXqFlq1\n\
[bat_mapper:from] south\n\
[bat_mapper:indoor] 1\n\
[bat_mapper:short] Monastery of Aeo\n\
[bat_mapper:long:0] You are just inside the monastery of Aeo in Dortlewall. The old monastery is\n\
[bat_mapper:long:1] to \x1b[38;5;10m\x1b[1msouth\x1b[0m\x1b[0m, and that is where you should head if you are here for the\n\
[bat_mapper:long:2] tutorial. To west you can see a small public library, and to east there is the\n\
[bat_mapper:long:3] new monastery, dug into the mountainside.\n\
[bat_mapper:exits] south,west,east\n"[..]));

        assert_eq!(bat_mapper.area, Some(String::from("dortlewall")));
        assert_eq!(bat_mapper.id, Some(String::from("$apr1$dF!!_X#W$FQb5R4zU.u6aIWJRXqFlq1")));
        assert_eq!(bat_mapper.short_desc, Some(String::from("Monastery of Aeo")));
        assert_eq!(bat_mapper.long_desc, Some(String::from("\
You are just inside the monastery of Aeo in Dortlewall. The old monastery is\n\
to \x1b[38;5;10m\x1b[1msouth\x1b[0m\x1b[0m, and that is where you should head if you are here for the\n\
tutorial. To west you can see a small public library, and to east there is the\n\
new monastery, dug into the mountainside.\n")));
        assert_eq!(bat_mapper.exits, Some(String::from("south,west,east")));
        assert_eq!(bat_mapper.is_indoor, Some(true));
        assert_eq!(bat_mapper.from_dir, Some(String::from("south")));
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

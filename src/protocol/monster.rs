use super::*;

#[derive(Debug)]
pub struct Monster {
    pub name: String,
    pub area: Option<String>,
    pub room_id: Option<String>,
    pub aggro: bool,
    pub output: BytesMut,
}

impl Monster {
    pub fn new(name: &BytesMut, area: Option<String>, room_id: Option<String>, aggro: bool) -> Monster {

        let output = if aggro {
            let mut bytes = BytesMut::with_capacity(16 + name.len());
            bytes.put(&b"[monster:aggro] "[..]);
            bytes.put(&name[..]);
            bytes
        } else {
            let mut bytes = BytesMut::with_capacity(11 + name.len());
            bytes.put(&b"[monster] "[..]);
            bytes.put(&name[..]);
            bytes
        };

        let name_len = name.len() - 6;
        Monster {
            name: latin1_to_string(&name.clone().split_off(6).split_to(name_len - 6)),
            area: area,
            room_id: room_id,
            aggro: aggro,
            output: output,
        }
    }
}

mod control_code;
mod bat_mapper;
mod monster;

use bytes::{BufMut, BytesMut};

pub use self::control_code::*;
pub use self::bat_mapper::*;
pub use self::monster::*;

pub fn usize_to_chars(mut x: usize) -> BytesMut {
    let mut bytes = BytesMut::with_capacity(3);
    while x >= 10 {
        bytes.put((x % 10) as u8 + b'0');
        x = x / 10;
    }
    bytes.put(x as u8 + b'0');
    bytes.reverse();
    bytes
}

pub fn latin1_to_string(bytes: &BytesMut) -> String {
    bytes.iter().map(|&c| c as char).collect()
}

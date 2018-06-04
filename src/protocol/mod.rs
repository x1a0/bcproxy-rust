mod control_code;
mod bat_mapper;

use bytes::{BufMut, BytesMut};

pub use self::control_code::*;
pub use self::bat_mapper::*;

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

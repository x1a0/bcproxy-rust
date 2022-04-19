use crate::bat_tag::BatTag;

use bytes::Bytes;

#[derive(Debug, PartialEq)]
pub enum BatFrame {
    Bytes(Bytes),
    Tag(BatTag),
}

impl From<BatFrame> for Bytes {
    fn from(frame: BatFrame) -> Self {
        match frame {
            BatFrame::Bytes(bytes) => bytes,
            BatFrame::Tag(tag) => tag.into(),
        }
    }
}

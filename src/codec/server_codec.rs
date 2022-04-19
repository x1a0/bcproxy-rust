use crate::codec::BatFrame;
use crate::bat_tag::BatTag;

use bytes::{Buf, BufMut, BytesMut, Bytes};
use std::{io::{self, ErrorKind}, str};
use tokio_util::codec::{Decoder, Encoder};

const NEW_LINE: u8 = 0x0A;
const ESC: u8 = 0x1b;


const IAC_SE: u8 = 0xf0;
const IAC_SB: u8 = 0xfa;
const IAC_WILL: u8 = 0xfb;
const IAC_DO: u8 = 0xfc;
const IAC_WONT: u8 = 0xfd;
const IAC_DONT: u8 = 0xfe;
const IAC: u8 = 0xff;

#[derive(Debug)]
pub enum ServerCodecError {
    Io(io::Error),
}

impl From<io::Error> for ServerCodecError {
    fn from(e: io::Error) -> ServerCodecError {
        ServerCodecError::Io(e)
    }
}

#[derive(Debug, PartialEq)]
enum DecoderState {
    Text,
    IAC,
    IACSub,
    ESC,
    Tag,
}

#[derive(Debug)]
pub struct ServerCodec {
    // Stored index of the next index to examine for characters that might
    // change decoder status.
    next_index: usize,

    // Stored the current decoding status.
    decoder_state: DecoderState,

    // Bat codes that are stacked for being framed.
    tags: Vec<BatTag>,
}

impl ServerCodec {
    pub fn new() -> ServerCodec {
        ServerCodec {
            next_index: 0,
            decoder_state: DecoderState::Text,
            tags: vec![],
        }
    }

    fn transit(&mut self, state: DecoderState) {
        self.decoder_state = state;
    }
}

fn utf8(buf: &[u8]) -> Result<&str, io::Error> {
    str::from_utf8(buf).map_err(|e| {
        log::error!("{}, {:?}", e, buf);
        io::Error::new(
            io::ErrorKind::InvalidData,
            "[server] Unable to decode input as UTF8",
        )
    })
}

fn without_carriage_return(mut bytes: BytesMut) -> BytesMut {
    if let Some(&b'\r') = bytes.last() {
        bytes.split_to(bytes.len() - 1)
    } else {
        bytes
    }
}

impl Decoder for ServerCodec {
    type Item = BatFrame;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<BatFrame>, Self::Error> {
        log::debug!("current buf: {:?}", buf);
        log::debug!("current codec: {:?}", self);
        loop {
            if buf.len() == 1 && buf[0] == b'\n' {
                let err = io::Error::new(ErrorKind::Other, "seems dead!");
                return Err(err);
            }

            match self.decoder_state {
                DecoderState::Text => {
                    // Look for next NEW_LINE, ESC or IAC. 
                    if let Some(offset) = buf[self.next_index..buf.len()].iter()
                        .position(|b| *b == NEW_LINE || *b == ESC || *b == IAC) {
                            let index = offset + self.next_index;
                            let c = buf[index];
                            match (index, c) {
                                (0, ESC) => {
                                    buf.advance(1);
                                    self.transit(DecoderState::ESC);

                                    if buf.is_empty() {
                                        return Ok(None);
                                    }
                                    continue;
                                }
                                (0, IAC) => {
                                    buf.advance(1);
                                    self.transit(DecoderState::IAC);

                                    if buf.is_empty() {
                                        return Ok(None);
                                    }
                                    continue;
                                }
                                (_, NEW_LINE) => {
                                    // take bytes until before the new line
                                    let bytes = buf.split_to(offset + self.next_index);
                                    buf.advance(1); // walk over the new line
                                    self.next_index = 0;
                                    self.transit(DecoderState::Text);
                                    let mut bytes = without_carriage_return(bytes);
                                    bytes.put_u8(b'\n');
                                    return Ok(Some(BatFrame::Bytes(bytes.freeze())));
                                }
                                (_, ESC) => {
                                    // take bytes until the char
                                    let bytes = buf.split_to(offset + self.next_index);
                                    self.next_index = 0;
                                    buf.advance(1);
                                    self.transit(DecoderState::ESC);
                                    return Ok(Some(BatFrame::Bytes(bytes.freeze())));
                                }
                                (_, IAC) => {
                                    // take bytes until the char
                                    let bytes = buf.split_to(offset + self.next_index);
                                    self.next_index = 0;
                                    buf.advance(1);
                                    self.transit(DecoderState::IAC);
                                    return Ok(Some(BatFrame::Bytes(bytes.freeze())));
                                }
                                _ => {
                                    let err = io::Error::new(ErrorKind::Other, "Should never happen!");
                                    return Err(err);
                                }
                            }
                        } else {
                            self.next_index = buf.len();
                            return Ok(None);
                        }
                }

                DecoderState::ESC => {
                    match buf.first() {
                        Some(b'<') if buf.len() < 3 => {
                            // Wait for more bytes to read the opening tag.
                            return Ok(None);
                        }
                        Some(b'<') => {
                            let code = (buf[1], buf[2]);
                            buf.advance(3);

                            let tag = BatTag::new(code);
                            self.tags.push(tag);
                            self.transit(DecoderState::Tag);

                            continue;
                        }
                        Some(b'>') if buf.len() < 3 => {
                            // Wait for more bytes to read the closing tag.
                            return Ok(None);
                        }
                        Some(b'>') => {
                            let code = (buf[1], buf[2]);
                            buf.advance(3);

                            if self.tags.len() == 0 {
                                // some tag closing for nothing
                                continue;
                            }

                            let tag = self.tags.last().unwrap();
                            if code != tag.code {
                                log::warn!(
                                    "Unmatched tag opening and closing. Opening: {}{}. Closing: {}{}.",
                                    tag.code.0 as char,
                                    tag.code.1 as char,
                                    code.0 as char,
                                    code.1 as char
                                );
                                // Discard the tag closing.
                                // Go back to tag parsing.
                                self.decoder_state = DecoderState::Tag;
                                continue;
                            }

                            let tag = self.tags.pop().unwrap();

                            if self.tags.is_empty() {
                                // All codes are closed - can be framed.
                                self.transit(DecoderState::Text);
                                return Ok(Some(BatFrame::Tag(tag)));
                            }

                            // Still in tag parsing. Pop current tag in the stack and
                            // add its bytes as content into the parent tag.
                            let bytes: Bytes = tag.into();
                            let tag = self.tags.last_mut().unwrap();
                            tag.content.put(bytes);
                            self.transit(DecoderState::Tag);

                            continue;
                        }
                        Some(b'|') => {
                            // A tag opening is ended by argument. What's stored as bat code
                            // content should be its argument.
                            let tag = self.tags.last_mut().unwrap();
                            tag.argument = Some(tag.content.clone().freeze());
                            tag.content.clear();
                            buf.advance(1);
                            self.transit(DecoderState::Tag);
                            continue;
                        }
                        Some(_) => {
                            // ESC that we do not interfer with.
                            if self.tags.is_empty() {
                                // If we are not parsing bat tag, forwarding it to client
                                // and continue processing in Text mode.
                                let frame = BatFrame::Bytes(Bytes::from(&[ESC][..]));
                                self.transit(DecoderState::Text);
                                return Ok(Some(frame));
                            } else {
                                // If we are parsing bat codes, push the ESC to tag content
                                // and carry one.
                                let tag = self.tags.last_mut().unwrap();
                                tag.content.put_u8(0x1b);
                                self.transit(DecoderState::Tag);
                                continue;
                            }
                        }
                        None => {
                            return Ok(None);
                        }
                    }
                }

                DecoderState::Tag => {
                    // Look for next ESC. 
                    let offset = buf[self.next_index..buf.len()]
                        .iter()
                        .position(|b| *b == ESC);

                    match offset {
                        Some(offset) => {
                            let index = offset + self.next_index;
                            let bytes = buf.split_to(index);
                            buf.advance(1);
                            self.next_index = 0;

                            // Store bytes as tag content for now. They might be tag arguments though.
                            let tag = self.tags.last_mut().unwrap();
                            tag.content.put(bytes);
                            self.transit(DecoderState::ESC);
                            continue;
                        }
                        None => {
                            self.next_index = buf.len();
                            return Ok(None);
                        }
                    }
                }

                DecoderState::IAC => {
                    if buf.is_empty() {
                        return Ok(None);
                    }

                    match buf[0] {
                        IAC_WILL | IAC_WONT | IAC_DO | IAC_DONT => {
                            if buf.len() < 2 {
                                // Wait for the 3rd byte
                                return Ok(None);
                            }

                            let bytes = Bytes::from(vec![IAC, buf[0], buf[1]]);
                            buf.advance(2);
                            self.decoder_state = DecoderState::Text;
                            return Ok(Some(BatFrame::Bytes(bytes)));
                        }
                        IAC_SB => {
                            // Subnegotiation of the indicated option follows.
                            // It should be closed by IAC_SE.
                            self.decoder_state = DecoderState::IACSub;
                            continue;
                        }
                        byte => {
                            let bytes = Bytes::from(vec![IAC, byte]);
                            buf.advance(1);
                            self.decoder_state = DecoderState::Text;
                            return Ok(Some(BatFrame::Bytes(bytes)));
                        }
                    }
                }

                DecoderState::IACSub => {
                    // Look for IAC_SE
                    if let Some(offset) = buf[self.next_index..buf.len()].iter()
                        .position(|b| *b == IAC_SE) {
                            let index = self.next_index + offset + 1;
                            let mut bytes = BytesMut::with_capacity(1 + index);
                            bytes.put_u8(IAC);
                            bytes.extend(buf.split_to(index));
                            self.decoder_state = DecoderState::Text;
                            return Ok(Some(BatFrame::Bytes(bytes.freeze())));
                        }
                }
            }
        }
    }

    /// At the end of the stream, try to decode the last frame. If cannot, put
    /// a `\n` character at the end and try again. If still no luck, log an
    /// error about it.
    fn decode_eof(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        Ok(match self.decode(buf)? {
            Some(frame) => Some(frame),
            None => {
                buf.put_u8(b'\n');
                match self.decode(buf)? {
                    Some(frame) => Some(frame),
                    None => {
                        log::error!("Failed to decode last frame. Bytes remaining: {:?}", buf);
                        None
                    }
                }
            }
        })
    }
}

impl<T> Encoder<T> for ServerCodec
where
    T: AsRef<[u8]>,
{
    type Error = std::io::Error;

    fn encode(&mut self, item: T, buf: &mut BytesMut) -> Result<(), Self::Error> {
        let bytes = item.as_ref();
        buf.reserve(bytes.len() + 1);
        buf.put(bytes);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! tag {
        ($code:expr, $argument:expr, $content:expr) => {
            {
                let mut tag = BatTag::new($code);
                tag.argument = $argument;
                tag.content = $content;
                tag
            }
        };
    }

    #[test]
    fn decoder_wait_for_newline() {
        let mut codec = ServerCodec::new();
        let mut buf = BytesMut::from("some text without newline");

        let frame = codec.decode(&mut buf);
        assert!(frame.is_ok());

        let frame = frame.unwrap();
        assert!(frame.is_none());

        assert_eq!(Bytes::from("some text without newline"), buf);
        assert_eq!(buf.len(), codec.next_index);
        assert_eq!(DecoderState::Text, codec.decoder_state);
    }

    #[test]
    fn decoder_frame_on_newline() {
        let mut codec = ServerCodec::new();
        let mut buf = BytesMut::from("some text with newline\n");

        let frame = codec.decode(&mut buf);
        assert!(frame.is_ok());

        let frame = frame.unwrap();
        assert!(frame.is_some());

        let expected = BatFrame::Bytes(Bytes::from("some text with newline\n"));
        let frame = frame.unwrap();
        assert_eq!(expected, frame);

        assert!(buf.is_empty());
        assert_eq!(0, codec.next_index);
        assert_eq!(DecoderState::Text, codec.decoder_state);
    }

    #[test]
    fn decoder_frame_on_newline_and_remove_carriage_return() {
        let mut codec = ServerCodec::new();
        let mut buf = BytesMut::from("some text\r\n");

        let frame = codec.decode(&mut buf);
        assert!(frame.is_ok());

        let frame = frame.unwrap();
        assert!(frame.is_some());

        let expected = BatFrame::Bytes(Bytes::from("some text\n"));
        let frame = frame.unwrap();
        assert_eq!(expected, frame);

        assert!(buf.is_empty());
        assert_eq!(0, codec.next_index);
        assert_eq!(DecoderState::Text, codec.decoder_state);
    }

    #[test]
    fn decoder_frame_bytes_before_bat_tag() {
        let mut codec = ServerCodec::new();
        let mut buf = BytesMut::from("some-text\x1b<00\x1b>00");

        let frame = codec.decode(&mut buf);
        assert!(frame.is_ok());

        let frame = frame.unwrap();
        assert!(frame.is_some());

        let expected = BatFrame::Bytes(Bytes::from("some-text"));
        let frame = frame.unwrap();
        assert_eq!(expected, frame);

        assert_eq!(buf, Bytes::from("<00\x1b>00"));
        assert_eq!(0, codec.next_index);
        assert_eq!(DecoderState::ESC, codec.decoder_state);
    }

    #[test]
    fn decoder_frame_single_bat_tag() {
        let mut codec = ServerCodec::new();
        let mut buf = BytesMut::from("\x1b<00\x1b>00");

        let frame = codec.decode(&mut buf);
        assert!(frame.is_ok());

        let frame = frame.unwrap();
        assert!(frame.is_some());

        let tag = tag!((b'0', b'0'), None, BytesMut::new());
        let expected = BatFrame::Tag(tag);
        let frame = frame.unwrap();
        assert_eq!(expected, frame);

        assert!(buf.is_empty());
        assert_eq!(0, codec.next_index);
        assert_eq!(DecoderState::Text, codec.decoder_state);
    }

    #[test]
    fn decoder_frame_single_bat_tag_in_multiple_runs() {
        let mut codec = ServerCodec::new();

        // run #1
        let mut buf = BytesMut::from("\x1b<540 1");
        let frame = codec.decode(&mut buf);
        assert!(frame.is_ok());

        let frame = frame.unwrap();
        assert!(frame.is_none());

        assert_eq!(buf, Bytes::from("0 1"));
        assert_eq!(buf.len(), codec.next_index);
        assert_eq!(DecoderState::Tag, codec.decoder_state);

        // run #2
        buf.put(&b"\x1b>54"[..]);
        let frame = codec.decode(&mut buf);
        assert!(frame.is_ok());

        let frame = frame.unwrap();
        assert!(frame.is_some());

        let tag = tag!((b'5', b'4'), None, BytesMut::from("0 1"));
        let expected = BatFrame::Tag(tag);
        let frame = frame.unwrap();
        assert_eq!(expected, frame);

        assert!(buf.is_empty());
        assert_eq!(0, codec.next_index);
        assert_eq!(DecoderState::Text, codec.decoder_state);
    }

    #[test]
    // A real world example
    fn decoder_frame_a_map_frame() {
        let mut codec = ServerCodec::new();
        let mut buf = BytesMut::from("\x1b<10spec_map\x1b|\x1b<11\x1b>11\x1b<200000FF\x1b|some text in blue color\x1b>20\x1b>10");

        let frame = codec.decode(&mut buf);
        assert!(frame.is_ok());

        let frame = frame.unwrap();
        assert!(frame.is_some());

        let tag1 = tag!((b'1', b'1'), None, BytesMut::new());
        let tag2 = tag!((b'2', b'0'), Some(Bytes::from("0000FF")), BytesMut::from("some text in blue color"));
        let mut bytes = BytesMut::new();
        bytes.extend(Bytes::from(tag1));
        bytes.extend(Bytes::from(tag2));
        let tag = tag!((b'1', b'0'), Some(Bytes::from("spec_map")), bytes);

        let expected = BatFrame::Tag(tag);
        let frame = frame.unwrap();
        assert_eq!(expected, frame);

        assert!(buf.is_empty());
        assert_eq!(0, codec.next_index);
        assert_eq!(DecoderState::Text, codec.decoder_state);
    }

    #[test]
    fn decoder_frame_bytes_before_iac() {
        let mut codec = ServerCodec::new();
        let mut buf = BytesMut::from("some text");
        buf.put_u8(IAC);
        buf.put_u8(0xf9);

        let frame = codec.decode(&mut buf);
        assert!(frame.is_ok());

        let frame = frame.unwrap();
        assert!(frame.is_some());

        let expected = BatFrame::Bytes(Bytes::from("some text"));
        let frame = frame.unwrap();
        assert_eq!(expected, frame);

        assert_eq!(buf, Bytes::from(vec![0xf9]));
        assert_eq!(0, codec.next_index);
        assert_eq!(DecoderState::IAC, codec.decoder_state);
    }

    #[test]
    fn decoder_frame_iac_2_bytes() {
        let mut codec = ServerCodec::new();
        let mut buf = BytesMut::from(&[IAC, 0xf9][..]);

        let frame = codec.decode(&mut buf);
        assert!(frame.is_ok());

        let frame = frame.unwrap();
        assert!(frame.is_some());

        let expected = BatFrame::Bytes(Bytes::from(vec![IAC, 0xf9]));
        let frame = frame.unwrap();
        assert_eq!(expected, frame);

        assert!(buf.is_empty());
        assert_eq!(0, codec.next_index);
        assert_eq!(DecoderState::Text, codec.decoder_state);
    }

    #[test]
    fn decoder_frame_iac_3_bytes() {
        let mut codec = ServerCodec::new();
        let mut buf = BytesMut::from(&[IAC, IAC_DO, 0x01][..]);

        let frame = codec.decode(&mut buf);
        assert!(frame.is_ok());

        let frame = frame.unwrap();
        assert!(frame.is_some());

        let expected = BatFrame::Bytes(Bytes::from(vec![IAC, IAC_DO, 0x01]));
        let frame = frame.unwrap();
        assert_eq!(expected, frame);

        assert!(buf.is_empty());
        assert_eq!(0, codec.next_index);
        assert_eq!(DecoderState::Text, codec.decoder_state);
    }

    #[test]
    fn decoder_frame_iac_sub_negotiation() {
        let mut codec = ServerCodec::new();
        // 255(IAC),250(SB),24,0,'V','T','2','2','0',255(IAC),240(SE)
        let mut buf = BytesMut::from(&[IAC, IAC_SB, 0x24, 0x00, b'V', b'T', b'2', b'2', b'0', IAC, IAC_SE][..]);

        let frame = codec.decode(&mut buf);
        assert!(frame.is_ok());

        let frame = frame.unwrap();
        assert!(frame.is_some());

        let expected = BatFrame::Bytes(Bytes::from(vec![IAC, IAC_SB, 0x24, 0x00, b'V', b'T', b'2', b'2', b'0', IAC, IAC_SE]));
        let frame = frame.unwrap();
        assert_eq!(expected, frame);

        assert!(buf.is_empty());
        assert_eq!(0, codec.next_index);
        assert_eq!(DecoderState::Text, codec.decoder_state);
    }

    #[test]
    fn decoder_frames_real_data() {
        let mut codec = ServerCodec::new();
        let mut buf = BytesMut::from("\x1b>20\x1b<20555555\x1b|#\x1b>20\x1b<20808400\x1b|.\x1b>20\x1b<20808400\x1b|.\x1b>20\x1b<20808400\x1b|.\x1b>20\x1b<20555555\x1b|#\x1b>20\x1b<20555555\x1b|#\x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20555555\x1b|#\x1b>20\x1b<20808400\x1b|.\x1b>20\x1b<20808400\x1b|.\x1b>20\x1b<20808400\x1b|.\x1b>20\x1b<20555555\x1b|#\x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20555555\x1b|#\x1b>20\x1b<20555555\x1b|#\x1b>20\x1b<20808400\x1b|.\x1b>20\x1b<20808400\x1b|.\x1b>20\x1b<20808400\x1b|.\x1b>20\x1b<20555555\x1b|#\x1b>20\r\n\x1b>20\x1b<20555555\x1b|#\x1b>20\x1b<20555555\x1b|#\x1b>20\x1b<20808400\x1b|.\x1b>20\x1b<20808400\x1b|.\x1b>20\x1b<20808400\x1b|.\x1b>20\x1b<20555555\x1b|#\x1b>20\x1b<20555555\x1b|#\x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20555555\x1b|#\x1b>20\x1b<20808400\x1b|.\x1b>20\x1b<20808400\x1b|.\x1b>20\x1b<20808400\x1b|.\x1b>20\x1b<20555555\x1b|#\x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20FFFF00\x1b| \x1b>20\x1b<20555555\x1b|#\x1b>20\x1b<20555555\x1b|#\x1b>20\x1b<20808400\x1b|.\x1b>20\x1b<20808400\x1b|.\x1b>20\x1b<2080840");

        while let Ok(Some(frame)) = codec.decode(&mut buf) {
            println!("{:?}", frame)
        }
    }
}

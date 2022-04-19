use std::u8;

use bytes::{Bytes, BytesMut, BufMut, Buf};

use super::FgBg;

// Converts RGB values to the nearest equivalent xterm-256 color.
// Inspired by https://github.com/lotheac/bcproxy

const BASE_COLORS: [u32; 16] = [
    0x000000,
    0x800000,
    0x008000,
    0x808000,
    0x000080,
    0x800080,
    0x008080,
    0xc0c0c0,
    0x808080,
    0xff0000,
    0x00ff00,
    0xffff00,
    0x0000ff,
    0xff00ff,
    0x00ffff,
    0xffffff,
];

const GREYS: [u32; 24] = [
    0x080808,
    0x121212,
    0x1c1c1c,
    0x262626,
    0x303030,
    0x3a3a3a,
    0x444444,
    0x4e4e4e,
    0x585858,
    0x626262,
    0x6c6c6c,
    0x767676,
    0x808080,
    0x8a8a8a,
    0x949494,
    0x9e9e9e,
    0xa8a8a8,
    0xb2b2b2,
    0xbcbcbc,
    0xc6c6c6,
    0xd0d0d0,
    0xdadada,
    0xe4e4e4,
    0xeeeeee,
];

macro_rules! scale {
    ($x:expr, $n:expr) => {
        (($x as u32) * $n / (u8::MAX as u32)) as u8
    }
}

pub fn colorize_content(content: &BytesMut, color: Vec<u8>, fg_bg: FgBg) -> Bytes {
    let color = match color.len() {
        n if n > 6 => {
            log::warn!("unexpected color attr: {:?}", color);
            0
        }
        _ => {
            let r: u8 = hex_to_u8(*color.get(0).unwrap_or(&0)) * 16
                + hex_to_u8(*color.get(1).unwrap_or(&0));
            let g: u8 = hex_to_u8(*color.get(2).unwrap_or(&0)) * 16
                + hex_to_u8(*color.get(3).unwrap_or(&0));
            let b: u8 = hex_to_u8(*color.get(4).unwrap_or(&0)) * 16
                + hex_to_u8(*color.get(5).unwrap_or(&0));
            rgb_to_xterm(r, g, b)
        }
    };

    // begin: ESC[(38|48);5;xxxm = 1+1+2+1+1+1+3+1 = 11
    // end: ESC[0m = 4
    let mut bytes = BytesMut::with_capacity(11 + 4 + content.len());
    let fg_bg = fg_bg.value();
    bytes.put_slice(format!("\x1b[{fg_bg};5;{color}m").as_bytes());
    bytes.extend(content);
    bytes.put_slice("\x1b[0m".as_bytes());
    bytes.freeze()
}

fn rgb_to_xterm(r: u8, g: u8, b: u8) -> u8 {
    let rgb: u32 = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);

    match BASE_COLORS.iter().position(|&x| x == rgb) {
        Some(i) => i as u8,

        None => match GREYS.iter().position(|&x| x == rgb) {
            Some(i) => 232 + i as u8,
            None => 16 + 36 * scale!(r, 5) + 6 * scale!(g, 5) + scale!(b, 5)
        },
    }
}

fn hex_to_u8(c: u8) -> u8 {
    match c {
        b'0' ..= b'9' => c - b'0',
        b'A' ..= b'F' => c - b'A' + 10,
        b'a' ..= b'f' => c - b'a' + 10,
        _ => 0
    }
}

#[cfg(test)]
mod test {
    use crate::color::FgBg;

    use super::*;

    #[test]
    fn foreground_color() {
        let content = BytesMut::from("content");
        let color = Bytes::from("EEEEEE").to_vec();
        let bytes = colorize_content(&content, color, FgBg::FG);
        assert_eq!(Bytes::from("\x1b[38;5;255mcontent\x1b[0m"), bytes);
    }

    #[test]
    fn background_color() {
        let content = BytesMut::from("content");
        let color = Bytes::from("EEEEEE").to_vec();
        let bytes = colorize_content(&content, color, FgBg::BG);
        assert_eq!(Bytes::from("\x1b[48;5;255mcontent\x1b[0m"), bytes);
    }

    #[test]
    fn base_colors() {
        // 16 base colors
        assert_eq!(rgb_to_xterm(0x00, 0x00, 0x00), 0);
        assert_eq!(rgb_to_xterm(0x80, 0x00, 0x00), 1);
        assert_eq!(rgb_to_xterm(0x00, 0x80, 0x00), 2);
        assert_eq!(rgb_to_xterm(0x80, 0x80, 0x00), 3);
        assert_eq!(rgb_to_xterm(0x00, 0x00, 0x80), 4);
        assert_eq!(rgb_to_xterm(0x80, 0x00, 0x80), 5);
        assert_eq!(rgb_to_xterm(0x00, 0x80, 0x80), 6);
        assert_eq!(rgb_to_xterm(0xc0, 0xc0, 0xc0), 7);
        assert_eq!(rgb_to_xterm(0x80, 0x80, 0x80), 8);
        assert_eq!(rgb_to_xterm(0xff, 0x00, 0x00), 9);
        assert_eq!(rgb_to_xterm(0x00, 0xff, 0x00), 10);
        assert_eq!(rgb_to_xterm(0xff, 0xff, 0x00), 11);
        assert_eq!(rgb_to_xterm(0x00, 0x00, 0xff), 12);
        assert_eq!(rgb_to_xterm(0xff, 0x00, 0xff), 13);
        assert_eq!(rgb_to_xterm(0x00, 0xff, 0xff), 14);
        assert_eq!(rgb_to_xterm(0xff, 0xff, 0xff), 15);
    }

    #[test]
    fn greys() {
        assert_eq!(rgb_to_xterm(0x08, 0x08, 0x08), 232);
        assert_eq!(rgb_to_xterm(0x12, 0x12, 0x12), 233);
        assert_eq!(rgb_to_xterm(0x1c, 0x1c, 0x1c), 234);
        assert_eq!(rgb_to_xterm(0x26, 0x26, 0x26), 235);
        assert_eq!(rgb_to_xterm(0x30, 0x30, 0x30), 236);
        assert_eq!(rgb_to_xterm(0x3a, 0x3a, 0x3a), 237);
        assert_eq!(rgb_to_xterm(0x44, 0x44, 0x44), 238);
        assert_eq!(rgb_to_xterm(0x4e, 0x4e, 0x4e), 239);
        assert_eq!(rgb_to_xterm(0x58, 0x58, 0x58), 240);
        assert_eq!(rgb_to_xterm(0x62, 0x62, 0x62), 241);
        assert_eq!(rgb_to_xterm(0x6c, 0x6c, 0x6c), 242);
        assert_eq!(rgb_to_xterm(0x76, 0x76, 0x76), 243);
        // assert_eq!(rgb_to_xterm(0x80, 0x80, 0x80), 244);
        assert_eq!(rgb_to_xterm(0x8a, 0x8a, 0x8a), 245);
        assert_eq!(rgb_to_xterm(0x94, 0x94, 0x94), 246);
        assert_eq!(rgb_to_xterm(0x9e, 0x9e, 0x9e), 247);
        assert_eq!(rgb_to_xterm(0xa8, 0xa8, 0xa8), 248);
        assert_eq!(rgb_to_xterm(0xb2, 0xb2, 0xb2), 249);
        assert_eq!(rgb_to_xterm(0xbc, 0xbc, 0xbc), 250);
        assert_eq!(rgb_to_xterm(0xc6, 0xc6, 0xc6), 251);
        assert_eq!(rgb_to_xterm(0xd0, 0xd0, 0xd0), 252);
        assert_eq!(rgb_to_xterm(0xda, 0xda, 0xda), 253);
        assert_eq!(rgb_to_xterm(0xe4, 0xe4, 0xe4), 254);
        assert_eq!(rgb_to_xterm(0xee, 0xee, 0xee), 255);
    }

    #[test]
    fn others() {
        assert_eq!(rgb_to_xterm(0x00, 0x00, 0x5f), 17);
        assert_eq!(rgb_to_xterm(0x00, 0x00, 0x87), 18);
        assert_eq!(rgb_to_xterm(0x00, 0x00, 0xaf), 19);
        assert_eq!(rgb_to_xterm(0x00, 0x00, 0xd7), 20);
        // assert_eq!(rgb_to_xterm(0x00, 0x00, 0xff), 21);
        assert_eq!(rgb_to_xterm(0x00, 0x5f, 0x00), 22);
        assert_eq!(rgb_to_xterm(0x00, 0x5f, 0x5f), 23);
        assert_eq!(rgb_to_xterm(0x00, 0x5f, 0x87), 24);
        assert_eq!(rgb_to_xterm(0x00, 0x5f, 0xaf), 25);
        assert_eq!(rgb_to_xterm(0x00, 0x5f, 0xd7), 26);
        assert_eq!(rgb_to_xterm(0x00, 0x5f, 0xff), 27);
        assert_eq!(rgb_to_xterm(0x00, 0x87, 0x00), 28);
        assert_eq!(rgb_to_xterm(0x00, 0x87, 0x5f), 29);
        assert_eq!(rgb_to_xterm(0x00, 0x87, 0x87), 30);
        assert_eq!(rgb_to_xterm(0x00, 0x87, 0xaf), 31);
        assert_eq!(rgb_to_xterm(0x00, 0x87, 0xd7), 32);
        assert_eq!(rgb_to_xterm(0x00, 0x87, 0xff), 33);
        assert_eq!(rgb_to_xterm(0x00, 0xaf, 0x00), 34);
        assert_eq!(rgb_to_xterm(0x00, 0xaf, 0x5f), 35);
        assert_eq!(rgb_to_xterm(0x00, 0xaf, 0x87), 36);
        assert_eq!(rgb_to_xterm(0x00, 0xaf, 0xaf), 37);
        assert_eq!(rgb_to_xterm(0x00, 0xaf, 0xd7), 38);
        assert_eq!(rgb_to_xterm(0x00, 0xaf, 0xff), 39);
        assert_eq!(rgb_to_xterm(0x00, 0xd7, 0x00), 40);
        assert_eq!(rgb_to_xterm(0x00, 0xd7, 0x5f), 41);
        assert_eq!(rgb_to_xterm(0x00, 0xd7, 0x87), 42);
        assert_eq!(rgb_to_xterm(0x00, 0xd7, 0xaf), 43);
        assert_eq!(rgb_to_xterm(0x00, 0xd7, 0xd7), 44);
        assert_eq!(rgb_to_xterm(0x00, 0xd7, 0xff), 45);
        //assert_eq!(rgb_to_xterm(0x00, 0xff, 0x00), 46);
        assert_eq!(rgb_to_xterm(0x00, 0xff, 0x5f), 47);
        assert_eq!(rgb_to_xterm(0x00, 0xff, 0x87), 48);
        assert_eq!(rgb_to_xterm(0x00, 0xff, 0xaf), 49);
        assert_eq!(rgb_to_xterm(0x00, 0xff, 0xd7), 50);
        //assert_eq!(rgb_to_xterm(0x00, 0xff, 0xff), 51);
        assert_eq!(rgb_to_xterm(0x5f, 0x00, 0x00), 52);
        assert_eq!(rgb_to_xterm(0x5f, 0x00, 0x5f), 53);
        assert_eq!(rgb_to_xterm(0x5f, 0x00, 0x87), 54);
        assert_eq!(rgb_to_xterm(0x5f, 0x00, 0xaf), 55);
        assert_eq!(rgb_to_xterm(0x5f, 0x00, 0xd7), 56);
        assert_eq!(rgb_to_xterm(0x5f, 0x00, 0xff), 57);
        assert_eq!(rgb_to_xterm(0x5f, 0x5f, 0x00), 58);
        assert_eq!(rgb_to_xterm(0x5f, 0x5f, 0x5f), 59);
        assert_eq!(rgb_to_xterm(0x5f, 0x5f, 0x87), 60);
        assert_eq!(rgb_to_xterm(0x5f, 0x5f, 0xaf), 61);
        assert_eq!(rgb_to_xterm(0x5f, 0x5f, 0xd7), 62);
        assert_eq!(rgb_to_xterm(0x5f, 0x5f, 0xff), 63);
        assert_eq!(rgb_to_xterm(0x5f, 0x87, 0x00), 64);
        assert_eq!(rgb_to_xterm(0x5f, 0x87, 0x5f), 65);
        assert_eq!(rgb_to_xterm(0x5f, 0x87, 0x87), 66);
        assert_eq!(rgb_to_xterm(0x5f, 0x87, 0xaf), 67);
        assert_eq!(rgb_to_xterm(0x5f, 0x87, 0xd7), 68);
        assert_eq!(rgb_to_xterm(0x5f, 0x87, 0xff), 69);
        assert_eq!(rgb_to_xterm(0x5f, 0xaf, 0x00), 70);
        assert_eq!(rgb_to_xterm(0x5f, 0xaf, 0x5f), 71);
        assert_eq!(rgb_to_xterm(0x5f, 0xaf, 0x87), 72);
        assert_eq!(rgb_to_xterm(0x5f, 0xaf, 0xaf), 73);
        assert_eq!(rgb_to_xterm(0x5f, 0xaf, 0xd7), 74);
        assert_eq!(rgb_to_xterm(0x5f, 0xaf, 0xff), 75);
        assert_eq!(rgb_to_xterm(0x5f, 0xd7, 0x00), 76);
        assert_eq!(rgb_to_xterm(0x5f, 0xd7, 0x5f), 77);
        assert_eq!(rgb_to_xterm(0x5f, 0xd7, 0x87), 78);
        assert_eq!(rgb_to_xterm(0x5f, 0xd7, 0xaf), 79);
        assert_eq!(rgb_to_xterm(0x5f, 0xd7, 0xd7), 80);
        assert_eq!(rgb_to_xterm(0x5f, 0xd7, 0xff), 81);
        assert_eq!(rgb_to_xterm(0x5f, 0xff, 0x00), 82);
        assert_eq!(rgb_to_xterm(0x5f, 0xff, 0x5f), 83);
        assert_eq!(rgb_to_xterm(0x5f, 0xff, 0x87), 84);
        assert_eq!(rgb_to_xterm(0x5f, 0xff, 0xaf), 85);
        assert_eq!(rgb_to_xterm(0x5f, 0xff, 0xd7), 86);
        assert_eq!(rgb_to_xterm(0x5f, 0xff, 0xff), 87);
        assert_eq!(rgb_to_xterm(0x87, 0x00, 0x00), 88);
        assert_eq!(rgb_to_xterm(0x87, 0x00, 0x5f), 89);
        assert_eq!(rgb_to_xterm(0x87, 0x00, 0x87), 90);
        assert_eq!(rgb_to_xterm(0x87, 0x00, 0xaf), 91);
        assert_eq!(rgb_to_xterm(0x87, 0x00, 0xd7), 92);
        assert_eq!(rgb_to_xterm(0x87, 0x00, 0xff), 93);
        assert_eq!(rgb_to_xterm(0x87, 0x5f, 0x00), 94);
        assert_eq!(rgb_to_xterm(0x87, 0x5f, 0x5f), 95);
        assert_eq!(rgb_to_xterm(0x87, 0x5f, 0x87), 96);
        assert_eq!(rgb_to_xterm(0x87, 0x5f, 0xaf), 97);
        assert_eq!(rgb_to_xterm(0x87, 0x5f, 0xd7), 98);
        assert_eq!(rgb_to_xterm(0x87, 0x5f, 0xff), 99);
        assert_eq!(rgb_to_xterm(0x87, 0x87, 0x00), 100);
        assert_eq!(rgb_to_xterm(0x87, 0x87, 0x5f), 101);
        assert_eq!(rgb_to_xterm(0x87, 0x87, 0x87), 102);
        assert_eq!(rgb_to_xterm(0x87, 0x87, 0xaf), 103);
        assert_eq!(rgb_to_xterm(0x87, 0x87, 0xd7), 104);
        assert_eq!(rgb_to_xterm(0x87, 0x87, 0xff), 105);
        assert_eq!(rgb_to_xterm(0x87, 0xaf, 0x00), 106);
        assert_eq!(rgb_to_xterm(0x87, 0xaf, 0x5f), 107);
        assert_eq!(rgb_to_xterm(0x87, 0xaf, 0x87), 108);
        assert_eq!(rgb_to_xterm(0x87, 0xaf, 0xaf), 109);
        assert_eq!(rgb_to_xterm(0x87, 0xaf, 0xd7), 110);
        assert_eq!(rgb_to_xterm(0x87, 0xaf, 0xff), 111);
        assert_eq!(rgb_to_xterm(0x87, 0xd7, 0x00), 112);
        assert_eq!(rgb_to_xterm(0x87, 0xd7, 0x5f), 113);
        assert_eq!(rgb_to_xterm(0x87, 0xd7, 0x87), 114);
        assert_eq!(rgb_to_xterm(0x87, 0xd7, 0xaf), 115);
        assert_eq!(rgb_to_xterm(0x87, 0xd7, 0xd7), 116);
        assert_eq!(rgb_to_xterm(0x87, 0xd7, 0xff), 117);
        assert_eq!(rgb_to_xterm(0x87, 0xff, 0x00), 118);
        assert_eq!(rgb_to_xterm(0x87, 0xff, 0x5f), 119);
        assert_eq!(rgb_to_xterm(0x87, 0xff, 0x87), 120);
        assert_eq!(rgb_to_xterm(0x87, 0xff, 0xaf), 121);
        assert_eq!(rgb_to_xterm(0x87, 0xff, 0xd7), 122);
        assert_eq!(rgb_to_xterm(0x87, 0xff, 0xff), 123);
        assert_eq!(rgb_to_xterm(0xaf, 0x00, 0x00), 124);
        assert_eq!(rgb_to_xterm(0xaf, 0x00, 0x5f), 125);
        assert_eq!(rgb_to_xterm(0xaf, 0x00, 0x87), 126);
        assert_eq!(rgb_to_xterm(0xaf, 0x00, 0xaf), 127);
        assert_eq!(rgb_to_xterm(0xaf, 0x00, 0xd7), 128);
        assert_eq!(rgb_to_xterm(0xaf, 0x00, 0xff), 129);
        assert_eq!(rgb_to_xterm(0xaf, 0x5f, 0x00), 130);
        assert_eq!(rgb_to_xterm(0xaf, 0x5f, 0x5f), 131);
        assert_eq!(rgb_to_xterm(0xaf, 0x5f, 0x87), 132);
        assert_eq!(rgb_to_xterm(0xaf, 0x5f, 0xaf), 133);
        assert_eq!(rgb_to_xterm(0xaf, 0x5f, 0xd7), 134);
        assert_eq!(rgb_to_xterm(0xaf, 0x5f, 0xff), 135);
        assert_eq!(rgb_to_xterm(0xaf, 0x87, 0x00), 136);
        assert_eq!(rgb_to_xterm(0xaf, 0x87, 0x5f), 137);
        assert_eq!(rgb_to_xterm(0xaf, 0x87, 0x87), 138);
        assert_eq!(rgb_to_xterm(0xaf, 0x87, 0xaf), 139);
        assert_eq!(rgb_to_xterm(0xaf, 0x87, 0xd7), 140);
        assert_eq!(rgb_to_xterm(0xaf, 0x87, 0xff), 141);
        assert_eq!(rgb_to_xterm(0xaf, 0xaf, 0x00), 142);
        assert_eq!(rgb_to_xterm(0xaf, 0xaf, 0x5f), 143);
        assert_eq!(rgb_to_xterm(0xaf, 0xaf, 0x87), 144);
        assert_eq!(rgb_to_xterm(0xaf, 0xaf, 0xaf), 145);
        assert_eq!(rgb_to_xterm(0xaf, 0xaf, 0xd7), 146);
        assert_eq!(rgb_to_xterm(0xaf, 0xaf, 0xff), 147);
        assert_eq!(rgb_to_xterm(0xaf, 0xd7, 0x00), 148);
        assert_eq!(rgb_to_xterm(0xaf, 0xd7, 0x5f), 149);
        assert_eq!(rgb_to_xterm(0xaf, 0xd7, 0x87), 150);
        assert_eq!(rgb_to_xterm(0xaf, 0xd7, 0xaf), 151);
        assert_eq!(rgb_to_xterm(0xaf, 0xd7, 0xd7), 152);
        assert_eq!(rgb_to_xterm(0xaf, 0xd7, 0xff), 153);
        assert_eq!(rgb_to_xterm(0xaf, 0xff, 0x00), 154);
        assert_eq!(rgb_to_xterm(0xaf, 0xff, 0x5f), 155);
        assert_eq!(rgb_to_xterm(0xaf, 0xff, 0x87), 156);
        assert_eq!(rgb_to_xterm(0xaf, 0xff, 0xaf), 157);
        assert_eq!(rgb_to_xterm(0xaf, 0xff, 0xd7), 158);
        assert_eq!(rgb_to_xterm(0xaf, 0xff, 0xff), 159);
        assert_eq!(rgb_to_xterm(0xd7, 0x00, 0x00), 160);
        assert_eq!(rgb_to_xterm(0xd7, 0x00, 0x5f), 161);
        assert_eq!(rgb_to_xterm(0xd7, 0x00, 0x87), 162);
        assert_eq!(rgb_to_xterm(0xd7, 0x00, 0xaf), 163);
        assert_eq!(rgb_to_xterm(0xd7, 0x00, 0xd7), 164);
        assert_eq!(rgb_to_xterm(0xd7, 0x00, 0xff), 165);
        assert_eq!(rgb_to_xterm(0xd7, 0x5f, 0x00), 166);
        assert_eq!(rgb_to_xterm(0xd7, 0x5f, 0x5f), 167);
        assert_eq!(rgb_to_xterm(0xd7, 0x5f, 0x87), 168);
        assert_eq!(rgb_to_xterm(0xd7, 0x5f, 0xaf), 169);
        assert_eq!(rgb_to_xterm(0xd7, 0x5f, 0xd7), 170);
        assert_eq!(rgb_to_xterm(0xd7, 0x5f, 0xff), 171);
        assert_eq!(rgb_to_xterm(0xd7, 0x87, 0x00), 172);
        assert_eq!(rgb_to_xterm(0xd7, 0x87, 0x5f), 173);
        assert_eq!(rgb_to_xterm(0xd7, 0x87, 0x87), 174);
        assert_eq!(rgb_to_xterm(0xd7, 0x87, 0xaf), 175);
        assert_eq!(rgb_to_xterm(0xd7, 0x87, 0xd7), 176);
        assert_eq!(rgb_to_xterm(0xd7, 0x87, 0xff), 177);
        assert_eq!(rgb_to_xterm(0xd7, 0xaf, 0x00), 178);
        assert_eq!(rgb_to_xterm(0xd7, 0xaf, 0x5f), 179);
        assert_eq!(rgb_to_xterm(0xd7, 0xaf, 0x87), 180);
        assert_eq!(rgb_to_xterm(0xd7, 0xaf, 0xaf), 181);
        assert_eq!(rgb_to_xterm(0xd7, 0xaf, 0xd7), 182);
        assert_eq!(rgb_to_xterm(0xd7, 0xaf, 0xff), 183);
        assert_eq!(rgb_to_xterm(0xd7, 0xd7, 0x00), 184);
        assert_eq!(rgb_to_xterm(0xd7, 0xd7, 0x5f), 185);
        assert_eq!(rgb_to_xterm(0xd7, 0xd7, 0x87), 186);
        assert_eq!(rgb_to_xterm(0xd7, 0xd7, 0xaf), 187);
        assert_eq!(rgb_to_xterm(0xd7, 0xd7, 0xd7), 188);
        assert_eq!(rgb_to_xterm(0xd7, 0xd7, 0xff), 189);
        assert_eq!(rgb_to_xterm(0xd7, 0xff, 0x00), 190);
        assert_eq!(rgb_to_xterm(0xd7, 0xff, 0x5f), 191);
        assert_eq!(rgb_to_xterm(0xd7, 0xff, 0x87), 192);
        assert_eq!(rgb_to_xterm(0xd7, 0xff, 0xaf), 193);
        assert_eq!(rgb_to_xterm(0xd7, 0xff, 0xd7), 194);
        assert_eq!(rgb_to_xterm(0xd7, 0xff, 0xff), 195);
        //assert_eq!(rgb_to_xterm(0xff, 0x00, 0x00), 196);
        assert_eq!(rgb_to_xterm(0xff, 0x00, 0x5f), 197);
        assert_eq!(rgb_to_xterm(0xff, 0x00, 0x87), 198);
        assert_eq!(rgb_to_xterm(0xff, 0x00, 0xaf), 199);
        assert_eq!(rgb_to_xterm(0xff, 0x00, 0xd7), 200);
        //assert_eq!(rgb_to_xterm(0xff, 0x00, 0xff), 201);
        assert_eq!(rgb_to_xterm(0xff, 0x5f, 0x00), 202);
        assert_eq!(rgb_to_xterm(0xff, 0x5f, 0x5f), 203);
        assert_eq!(rgb_to_xterm(0xff, 0x5f, 0x87), 204);
        assert_eq!(rgb_to_xterm(0xff, 0x5f, 0xaf), 205);
        assert_eq!(rgb_to_xterm(0xff, 0x5f, 0xd7), 206);
        assert_eq!(rgb_to_xterm(0xff, 0x5f, 0xff), 207);
        assert_eq!(rgb_to_xterm(0xff, 0x87, 0x00), 208);
        assert_eq!(rgb_to_xterm(0xff, 0x87, 0x5f), 209);
        assert_eq!(rgb_to_xterm(0xff, 0x87, 0x87), 210);
        assert_eq!(rgb_to_xterm(0xff, 0x87, 0xaf), 211);
        assert_eq!(rgb_to_xterm(0xff, 0x87, 0xd7), 212);
        assert_eq!(rgb_to_xterm(0xff, 0x87, 0xff), 213);
        assert_eq!(rgb_to_xterm(0xff, 0xaf, 0x00), 214);
        assert_eq!(rgb_to_xterm(0xff, 0xaf, 0x5f), 215);
        assert_eq!(rgb_to_xterm(0xff, 0xaf, 0x87), 216);
        assert_eq!(rgb_to_xterm(0xff, 0xaf, 0xaf), 217);
        assert_eq!(rgb_to_xterm(0xff, 0xaf, 0xd7), 218);
        assert_eq!(rgb_to_xterm(0xff, 0xaf, 0xff), 219);
        assert_eq!(rgb_to_xterm(0xff, 0xd7, 0x00), 220);
        assert_eq!(rgb_to_xterm(0xff, 0xd7, 0x5f), 221);
        assert_eq!(rgb_to_xterm(0xff, 0xd7, 0x87), 222);
        assert_eq!(rgb_to_xterm(0xff, 0xd7, 0xaf), 223);
        assert_eq!(rgb_to_xterm(0xff, 0xd7, 0xd7), 224);
        assert_eq!(rgb_to_xterm(0xff, 0xd7, 0xff), 225);
        //assert_eq!(rgb_to_xterm(0xff, 0xff, 0x00), 226);
        assert_eq!(rgb_to_xterm(0xff, 0xff, 0x5f), 227);
        assert_eq!(rgb_to_xterm(0xff, 0xff, 0x87), 228);
        assert_eq!(rgb_to_xterm(0xff, 0xff, 0xaf), 229);
        assert_eq!(rgb_to_xterm(0xff, 0xff, 0xd7), 230);
    }
}

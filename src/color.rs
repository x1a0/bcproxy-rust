use std::u8;

use bytes::{BytesMut, BufMut};

/// Converts RGB values to the nearest equivalent xterm-256 color.
/// Inspired by https://github.com/lotheac/bcproxy

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

pub fn rgb_to_xterm(mut bytes: BytesMut) -> u8 {
    let len = bytes.len();

    if len > 6 {
        warn!("unexpected color attr: {:?}", bytes);
        return 0;
    }

    if len < 6 {
        bytes.reserve(6 - len);
        bytes.put_slice(&[b'0'; 6][len..]);
    }

    let r: u8 = hex_to_u8(bytes[0]) * 16 + hex_to_u8(bytes[1]);
    let g: u8 = hex_to_u8(bytes[2]) * 16 + hex_to_u8(bytes[3]);
    let b: u8 = hex_to_u8(bytes[4]) * 16 + hex_to_u8(bytes[5]);

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
    use super::*;

    #[test]
    fn base_colors() {
        // 16 base colors
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"000000"[..])), 0);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"800000"[..])), 1);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"008000"[..])), 2);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"808000"[..])), 3);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"000080"[..])), 4);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"800080"[..])), 5);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"008080"[..])), 6);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"c0c0c0"[..])), 7);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"808080"[..])), 8);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ff0000"[..])), 9);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"00ff00"[..])), 10);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ffff00"[..])), 11);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"0000ff"[..])), 12);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ff00ff"[..])), 13);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"00ffff"[..])), 14);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ffffff"[..])), 15);
    }

    #[test]
    fn greys() {
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"080808"[..])), 232);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"121212"[..])), 233);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"1c1c1c"[..])), 234);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"262626"[..])), 235);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"303030"[..])), 236);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"3a3a3a"[..])), 237);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"444444"[..])), 238);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"4e4e4e"[..])), 239);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"585858"[..])), 240);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"626262"[..])), 241);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"6c6c6c"[..])), 242);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"767676"[..])), 243);
        // assert_eq!(rgb_to_xterm(BytesMut::from(&b"808080"[..])), 244);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"8a8a8a"[..])), 245);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"949494"[..])), 246);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"9e9e9e"[..])), 247);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"a8a8a8"[..])), 248);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"b2b2b2"[..])), 249);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"bcbcbc"[..])), 250);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"c6c6c6"[..])), 251);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d0d0d0"[..])), 252);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"dadada"[..])), 253);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"e4e4e4"[..])), 254);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"eeeeee"[..])), 255);
    }

    #[test]
    fn others() {
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"00005f"[..])), 17);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"000087"[..])), 18);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"0000af"[..])), 19);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"0000d7"[..])), 20);
        // assert_eq!(rgb_to_xterm(BytesMut::from(&b"0000ff"[..])), 21);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"005f00"[..])), 22);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"005f5f"[..])), 23);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"005f87"[..])), 24);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"005faf"[..])), 25);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"005fd7"[..])), 26);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"005fff"[..])), 27);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"008700"[..])), 28);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"00875f"[..])), 29);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"008787"[..])), 30);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"0087af"[..])), 31);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"0087d7"[..])), 32);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"0087ff"[..])), 33);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"00af00"[..])), 34);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"00af5f"[..])), 35);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"00af87"[..])), 36);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"00afaf"[..])), 37);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"00afd7"[..])), 38);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"00afff"[..])), 39);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"00d700"[..])), 40);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"00d75f"[..])), 41);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"00d787"[..])), 42);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"00d7af"[..])), 43);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"00d7d7"[..])), 44);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"00d7ff"[..])), 45);
        //assert_eq!(rgb_to_xterm(BytesMut::from(&b"00ff00"[..])), 46);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"00ff5f"[..])), 47);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"00ff87"[..])), 48);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"00ffaf"[..])), 49);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"00ffd7"[..])), 50);
        //assert_eq!(rgb_to_xterm(BytesMut::from(&b"00ffff"[..])), 51);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5f0000"[..])), 52);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5f005f"[..])), 53);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5f0087"[..])), 54);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5f00af"[..])), 55);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5f00d7"[..])), 56);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5f00ff"[..])), 57);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5f5f00"[..])), 58);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5f5f5f"[..])), 59);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5f5f87"[..])), 60);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5f5faf"[..])), 61);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5f5fd7"[..])), 62);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5f5fff"[..])), 63);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5f8700"[..])), 64);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5f875f"[..])), 65);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5f8787"[..])), 66);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5f87af"[..])), 67);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5f87d7"[..])), 68);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5f87ff"[..])), 69);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5faf00"[..])), 70);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5faf5f"[..])), 71);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5faf87"[..])), 72);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5fafaf"[..])), 73);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5fafd7"[..])), 74);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5fafff"[..])), 75);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5fd700"[..])), 76);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5fd75f"[..])), 77);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5fd787"[..])), 78);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5fd7af"[..])), 79);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5fd7d7"[..])), 80);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5fd7ff"[..])), 81);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5fff00"[..])), 82);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5fff5f"[..])), 83);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5fff87"[..])), 84);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5fffaf"[..])), 85);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5fffd7"[..])), 86);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"5fffff"[..])), 87);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"870000"[..])), 88);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"87005f"[..])), 89);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"870087"[..])), 90);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"8700af"[..])), 91);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"8700d7"[..])), 92);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"8700ff"[..])), 93);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"875f00"[..])), 94);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"875f5f"[..])), 95);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"875f87"[..])), 96);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"875faf"[..])), 97);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"875fd7"[..])), 98);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"875fff"[..])), 99);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"878700"[..])), 100);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"87875f"[..])), 101);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"878787"[..])), 102);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"8787af"[..])), 103);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"8787d7"[..])), 104);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"8787ff"[..])), 105);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"87af00"[..])), 106);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"87af5f"[..])), 107);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"87af87"[..])), 108);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"87afaf"[..])), 109);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"87afd7"[..])), 110);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"87afff"[..])), 111);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"87d700"[..])), 112);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"87d75f"[..])), 113);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"87d787"[..])), 114);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"87d7af"[..])), 115);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"87d7d7"[..])), 116);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"87d7ff"[..])), 117);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"87ff00"[..])), 118);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"87ff5f"[..])), 119);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"87ff87"[..])), 120);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"87ffaf"[..])), 121);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"87ffd7"[..])), 122);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"87ffff"[..])), 123);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"af0000"[..])), 124);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"af005f"[..])), 125);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"af0087"[..])), 126);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"af00af"[..])), 127);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"af00d7"[..])), 128);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"af00ff"[..])), 129);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"af5f00"[..])), 130);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"af5f5f"[..])), 131);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"af5f87"[..])), 132);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"af5faf"[..])), 133);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"af5fd7"[..])), 134);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"af5fff"[..])), 135);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"af8700"[..])), 136);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"af875f"[..])), 137);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"af8787"[..])), 138);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"af87af"[..])), 139);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"af87d7"[..])), 140);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"af87ff"[..])), 141);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"afaf00"[..])), 142);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"afaf5f"[..])), 143);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"afaf87"[..])), 144);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"afafaf"[..])), 145);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"afafd7"[..])), 146);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"afafff"[..])), 147);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"afd700"[..])), 148);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"afd75f"[..])), 149);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"afd787"[..])), 150);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"afd7af"[..])), 151);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"afd7d7"[..])), 152);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"afd7ff"[..])), 153);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"afff00"[..])), 154);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"afff5f"[..])), 155);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"afff87"[..])), 156);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"afffaf"[..])), 157);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"afffd7"[..])), 158);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"afffff"[..])), 159);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d70000"[..])), 160);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d7005f"[..])), 161);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d70087"[..])), 162);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d700af"[..])), 163);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d700d7"[..])), 164);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d700ff"[..])), 165);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d75f00"[..])), 166);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d75f5f"[..])), 167);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d75f87"[..])), 168);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d75faf"[..])), 169);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d75fd7"[..])), 170);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d75fff"[..])), 171);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d78700"[..])), 172);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d7875f"[..])), 173);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d78787"[..])), 174);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d787af"[..])), 175);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d787d7"[..])), 176);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d787ff"[..])), 177);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d7af00"[..])), 178);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d7af5f"[..])), 179);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d7af87"[..])), 180);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d7afaf"[..])), 181);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d7afd7"[..])), 182);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d7afff"[..])), 183);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d7d700"[..])), 184);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d7d75f"[..])), 185);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d7d787"[..])), 186);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d7d7af"[..])), 187);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d7d7d7"[..])), 188);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d7d7ff"[..])), 189);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d7ff00"[..])), 190);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d7ff5f"[..])), 191);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d7ff87"[..])), 192);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d7ffaf"[..])), 193);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d7ffd7"[..])), 194);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"d7ffff"[..])), 195);
        //assert_eq!(rgb_to_xterm(BytesMut::from(&b"ff0000"[..])), 196);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ff005f"[..])), 197);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ff0087"[..])), 198);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ff00af"[..])), 199);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ff00d7"[..])), 200);
        //assert_eq!(rgb_to_xterm(BytesMut::from(&b"ff00ff"[..])), 201);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ff5f00"[..])), 202);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ff5f5f"[..])), 203);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ff5f87"[..])), 204);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ff5faf"[..])), 205);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ff5fd7"[..])), 206);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ff5fff"[..])), 207);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ff8700"[..])), 208);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ff875f"[..])), 209);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ff8787"[..])), 210);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ff87af"[..])), 211);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ff87d7"[..])), 212);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ff87ff"[..])), 213);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ffaf00"[..])), 214);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ffaf5f"[..])), 215);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ffaf87"[..])), 216);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ffafaf"[..])), 217);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ffafd7"[..])), 218);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ffafff"[..])), 219);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ffd700"[..])), 220);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ffd75f"[..])), 221);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ffd787"[..])), 222);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ffd7af"[..])), 223);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ffd7d7"[..])), 224);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ffd7ff"[..])), 225);
        //assert_eq!(rgb_to_xterm(BytesMut::from(&b"ffff00"[..])), 226);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ffff5f"[..])), 227);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ffff87"[..])), 228);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ffffaf"[..])), 229);
        assert_eq!(rgb_to_xterm(BytesMut::from(&b"ffffd7"[..])), 230);
    }
}

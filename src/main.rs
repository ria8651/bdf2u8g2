// 今日は

use bdf_parser::Property;
use clap::{Arg, ArgAction, Command};
use std::{collections::HashSet, fs, path::PathBuf};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

fn main() {
    let app = Command::new("convert-bdf")
         .arg_required_else_help(true)
        .about(
r#"Generate embedded-graphic accepted Glyphs from bdf fonts file. 
if exist multi range* options at the same time. merge them as final exporting glyphs scope"
"#
        )
        .arg(
            Arg::new("input")
                .long("bdffile")
                .help("Input bdf file")
                .short('i')
                .value_parser(clap::value_parser!(PathBuf))
                .required(true)
                .action(ArgAction::Set)
                .value_name("FILE"),
        )
        .arg(
            Arg::new("output")
                .long("output")
                .help("output path. if not exist \".rs\" extention in it, will look it as dirctory, and use the bdf file's stem as its stem.")
                .short('o')
                .value_parser(clap::value_parser!(PathBuf))
                .default_value("./")
                .action(ArgAction::Set)
                .value_name("PATH"),
        )
        .arg(
            Arg::new("range")
                .long("range")
                .help(
r#"export characters list,defaultly export all glyphs in the bdf. e.g --range "abc" means only export a,b and c code's glyphs. 
"#
                )
                .action(ArgAction::Append)
                .value_name("RANGE"),
        )
        .arg(
            Arg::new("range-file")
                .long("range-file")
                .help(
                   r#"same as range option, but through characters file."#
                )
                .value_parser(clap::value_parser!(PathBuf))
                .action(ArgAction::Append)
                .value_name("RANGEFILE"),
        )
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"));

    let matches = app.get_matches();

    let bdf_file = matches.get_one::<PathBuf>("input").unwrap();
    if bdf_file.is_file() == false {
        println!("bdf file not exist");
        return;
    }

    let output = matches.get_one::<PathBuf>("output").unwrap().clone();

    let mut char_set = Vec::<char>::new();
    if let Some(paths) = matches.get_many::<PathBuf>("range-file") {
        for p in paths.collect::<Vec<_>>() {
            if p.is_file() {
                for c in fs::read_to_string(p)
                    .expect("couldn't open BDF file")
                    .chars()
                {
                    if c != '\r' && c != '\n' {
                        char_set.push(c);
                    }
                }
            } else {
                println!("input range file is not exist, ignore it:{:?}", p);
            }
        }
    }
    if let Some(ss) = matches.get_many::<String>("range") {
        for p in ss.collect::<Vec<_>>() {
            char_set.extend(p.chars());
        }
    }
    char_set.sort();
    char_set.dedup();

    let bdf = fs::read(bdf_file.as_path()).expect("couldn't open BDF file");
    let font = bdf_parser::BdfFont::parse(&bdf).expect("BDF file is bad format");

    let mut hash_char_set = HashSet::new();
    for &c in char_set.iter() {
        hash_char_set.insert(c);
    }

    let run_bits_0 = 2;
    let run_bits_1 = 2;

    let mut short_glyphs: Vec<Glyph> = Vec::new();
    let mut long_glyphs: Vec<Glyph> = Vec::new();
    print!("using glyphs: ");
    for glyph in font.glyphs.iter() {
        let Some(c) = glyph.encoding else {
            continue;
        };
        if !hash_char_set.contains(&c) && !char_set.is_empty() {
            continue;
        }
        hash_char_set.remove(&c);

        print!("{}", c);

        // unicode
        let unicode = if (c as u32) < 0x0100 {
            Unicode::Single(c as u8)
        } else if (c as u32) < 0x10000 {
            Unicode::Double(c as u16)
        } else {
            panic!("unicode too large");
        };

        // width, height, offset and advance
        let bb = &glyph.bounding_box;
        let width: u32 = bb.size.x.try_into().expect("width not positive?");
        let height: u32 = bb.size.y.try_into().expect("height not positive?");
        // let offset_x = bb.offset.x;
        // let offset_y = bb.offset.y;
        let offset_x = 0;
        let offset_y = 0;
        // let advance = glyph.device_width.x;
        // let advance = width as i32;
        let advance = 0;
        // println!("{}: {:?}", c, advance);

        // bitmap
        let mut length_pairs: Vec<(u32, u32)> = Vec::new();
        let mut run_0 = 0;
        let mut run_1 = 0;
        // println!("{}:", c);
        // for y in 0..height {
        //     for x in 0..width {
        //         let pixel = glyph.pixel(x as usize, y as usize);
        //         if pixel {
        //             print!("#");
        //         } else {
        //             print!(".");
        //         }
        //     }
        //     println!();
        // }
        // while let Some(bit) = reader.read_bit() {
        //     if bit {
        //         if run_1 + 1 >= (1 << run_bits_1) {
        //             length_pairs.push((run_0, run_1));
        //             run_0 = 0;
        //             run_1 = 0;
        //         }

        //         run_1 += 1;
        //     } else {
        //         if run_1 > 0 {
        //             length_pairs.push((run_0, run_1));
        //             run_0 = 0;
        //             run_1 = 0;
        //         }
        //         if run_0 + 1 >= (1 << run_bits_0) {
        //             length_pairs.push((run_0, run_1));
        //             run_0 = 0;
        //             run_1 = 0;
        //         }

        //         run_0 += 1;
        //     }
        // }
        for y in 0..height {
            for x in 0..width {
                let bit = glyph.pixel(x as usize, y as usize);
                if bit {
                    if run_1 + 1 >= (1 << run_bits_1) {
                        length_pairs.push((run_0, run_1));
                        run_0 = 0;
                        run_1 = 0;
                    }

                    run_1 += 1;
                } else {
                    if run_1 > 0 {
                        length_pairs.push((run_0, run_1));
                        run_0 = 0;
                        run_1 = 0;
                    }
                    if run_0 + 1 >= (1 << run_bits_0) {
                        length_pairs.push((run_0, run_1));
                        run_0 = 0;
                        run_1 = 0;
                    }

                    run_0 += 1;
                }
            }
        }
        length_pairs.push((run_0, run_1));

        // ((number of 0s, number of 1s), number of times to repeat)
        let mut combined_length_pairs: Vec<((u32, u32), u32)> = Vec::new();
        let mut last = *length_pairs.get(0).expect("bruh");
        let mut repeat = 0;
        for &pair in length_pairs[1..].iter() {
            assert!(pair.0 < (1 << run_bits_1));
            assert!(pair.1 < (1 << run_bits_0));
            if last == pair {
                repeat += 1;
            } else {
                combined_length_pairs.push((last, repeat));
                last = pair;
                repeat = 0;
            }
        }
        combined_length_pairs.push((last, repeat));

        let glyph = Glyph {
            unicode: unicode.clone(),
            width,
            height,
            offset_x,
            offset_y,
            advance,
            bitmap: combined_length_pairs,
        };
        if matches!(unicode, Unicode::Single(_)) {
            short_glyphs.push(glyph);
        } else {
            long_glyphs.push(glyph);
        }
    }
    println!();

    // header
    let header = U8g2Header {
        glyph_cnt: 0,
        bbx_mode: 0,
        bits_per_0: run_bits_0,
        bits_per_1: run_bits_1,
        bits_per_char_width: 5,
        bits_per_char_height: 5,
        bits_per_char_x: 4,
        bits_per_char_y: 5,
        bits_per_advance: 5,
        max_char_width: font.metadata.bounding_box.size.x.try_into().unwrap(),
        max_char_height: font.metadata.bounding_box.size.y.try_into().unwrap(),
        // x_offset: font.metadata.bounding_box.offset.x.try_into().unwrap(),
        // y_offset: font.metadata.bounding_box.offset.y.try_into().unwrap(),
        // ascent_a: font
        //     .properties
        //     .try_get::<i32>(Property::FontAscent)
        //     .unwrap()
        //     .try_into()
        //     .unwrap(),
        // descent_g: font
        //     .properties
        //     .try_get::<i32>(Property::FontDescent)
        //     .unwrap()
        //     .try_into()
        //     .unwrap(),
        // ascent_para: 10,  // TODO
        // descent_para: -1, // TODO
        x_offset: 0,
        y_offset: 0,
        ascent_a: 0,
        descent_g: 0,
        ascent_para: 0,
        descent_para: 0,
        start_pos_upper_a: 0,
        start_pos_lower_a: 0,
        start_pos_unicode: 0,
    };

    let mut u8g2_file = U8g2File {
        header,
        short_glyphs,
        long_glyphs,
    };
    fs::write(output, &u8g2_file.to_bytes()).expect("couldn't write to output file");
}

struct U8g2File {
    header: U8g2Header,
    short_glyphs: Vec<Glyph>,
    long_glyphs: Vec<Glyph>,
}

const U8G2_HEADER_SIZE: usize = std::mem::size_of::<U8g2Header>();

impl U8g2File {
    fn to_bytes(&mut self) -> Vec<u8> {
        // glyphs
        let mut bytes = Vec::new();

        // short glyphs
        for i in 0..self.short_glyphs.len() {
            let glyph = &self.short_glyphs[i];

            // if glyph.unicode == Unicode::Single(65) {
            //     // capital A
            //     println!("{:?}", &glyph);

            //     let mut reproduced_bits = Vec::new();
            //     for &((zeros, ones), repeat) in glyph.bitmap.iter() {
            //         for _ in 0..repeat + 1 {
            //             reproduced_bits.extend_from_slice(&vec![false; zeros as usize]);
            //             reproduced_bits.extend_from_slice(&vec![true; ones as usize]);
            //         }
            //     }

            //     for (i, bit) in reproduced_bits.into_iter().enumerate() {
            //         if i % glyph.width as usize == 0 {
            //             print!("\n");
            //         }
            //         print!("{}", if bit { '1' } else { '0' });
            //     }
            // }

            let no_jump = i == self.short_glyphs.len() - 1;
            bytes.extend_from_slice(&glyph.to_bytes(&self.header, no_jump));
        }

        // jump table
        let long_offset = bytes.len();
        let jump: u16 = u16::try_from(4).unwrap();
        let final_unicode: u16 = 0xffff;
        bytes.extend_from_slice(&jump.to_be_bytes());
        bytes.extend_from_slice(&final_unicode.to_be_bytes());

        // long glyphs
        for i in 0..self.long_glyphs.len() {
            let glyph = &self.long_glyphs[i];
            let no_jump = i == self.long_glyphs.len() - 1;
            bytes.extend_from_slice(&glyph.to_bytes(&self.header, no_jump));
        }

        // serves as test that font file is searchable
        print!("checking font file, found: ");
        let mut offset = 0;
        let mut upper_a = None;
        let mut lower_a = None;
        loop {
            let codepoint = bytes[offset];
            if codepoint == b'A' {
                upper_a = Some(offset);
            } else if codepoint == b'a' {
                lower_a = Some(offset);
            }
            print!("{}", codepoint as char);

            let jump = bytes[offset + 1];
            if jump == 0 {
                break;
            }

            offset += jump as usize;
        }
        // offset = bytes[long_offset]; // jump table
        offset = long_offset + 4;
        loop {
            let codepoint = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]);
            print!("{}", char::from_u32(codepoint as u32).unwrap());

            let jump = bytes[offset + 2];
            if jump == 0 {
                break;
            }

            offset += jump as usize;
        }
        println!();

        println!("A: {:x?}", &bytes[upper_a.unwrap()..upper_a.unwrap() + 32]);

        // change header glyph indices to byte indices
        self.header.glyph_cnt = (self.short_glyphs.len() + self.long_glyphs.len()) as u8;
        self.header.start_pos_upper_a = upper_a.unwrap().try_into().unwrap();
        self.header.start_pos_lower_a = lower_a.unwrap().try_into().unwrap();
        self.header.start_pos_unicode = long_offset.try_into().unwrap();

        // make two byte header variables big-endian
        self.header.start_pos_upper_a = self.header.start_pos_upper_a.swap_bytes();
        self.header.start_pos_lower_a = self.header.start_pos_lower_a.swap_bytes();
        self.header.start_pos_unicode = self.header.start_pos_unicode.swap_bytes();

        // insert header
        assert!(U8G2_HEADER_SIZE == 23);
        let h = self.header.as_bytes();
        assert!(U8G2_HEADER_SIZE == h.len());
        println!("header: {:x?}", h);
        bytes.splice(0..0, h.iter().copied());

        bytes
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, IntoBytes, FromBytes, KnownLayout, Immutable)]
#[repr(packed)]
struct U8g2Header {
    glyph_cnt: u8,
    bbx_mode: u8,
    bits_per_0: u8,
    bits_per_1: u8,
    bits_per_char_width: u8,
    bits_per_char_height: u8,
    bits_per_char_x: u8,
    bits_per_char_y: u8,
    bits_per_advance: u8,
    max_char_width: i8,
    max_char_height: i8,
    x_offset: i8,
    y_offset: i8,
    ascent_a: i8,
    descent_g: i8,
    ascent_para: i8,
    descent_para: i8,
    start_pos_upper_a: u16,
    start_pos_lower_a: u16,
    start_pos_unicode: u16,
}

#[derive(Debug)]
struct Glyph {
    unicode: Unicode,
    width: u32,
    height: u32,
    offset_x: i32,
    offset_y: i32,
    advance: u32,
    bitmap: Vec<((u32, u32), u32)>,
}

impl Glyph {
    fn to_bytes(&self, header: &U8g2Header, no_jump: bool) -> Vec<u8> {
        let mut bits = BitArray::new();

        // unicode
        let jump_pos = match self.unicode {
            Unicode::Single(c) => {
                bits.add_bits(c, 8).unwrap();
                1
            }
            Unicode::Double(c) => {
                // have to make sure that the unicode is in big-endian
                bits.add_bits(c, 16).unwrap();
                2
            }
        };

        // jump
        bits.add_bits(0, 8).unwrap();

        // rest of header
        bits.add_bits(self.width, header.bits_per_char_width)
            .unwrap();
        bits.add_bits(self.height, header.bits_per_char_height)
            .unwrap();
        bits.add_bits(self.offset_x, header.bits_per_char_x)
            .unwrap();
        bits.add_bits(self.offset_y, header.bits_per_char_y)
            .unwrap();
        bits.add_bits(self.advance, header.bits_per_advance)
            .unwrap();

        // bitmap
        for &((zeros, ones), repeat) in self.bitmap.iter() {
            bits.add_bits(zeros, header.bits_per_0).unwrap();
            bits.add_bits(ones, header.bits_per_1).unwrap();
            for _ in 0..repeat {
                bits.push(true);
            }
            bits.push(false);
        }

        // padding
        while bits.len() % 8 != 0 {
            bits.push(false);
        }

        // write to bytes
        let mut bytes = bits.to_bytes();

        // update jump
        if !no_jump {
            bytes[jump_pos] = (bytes.len()).try_into().unwrap();
        }

        bytes
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Unicode {
    Single(u8),
    Double(u16),
}

struct BitArray {
    bits: Vec<bool>,
}

impl BitArray {
    fn new() -> Self {
        Self { bits: Vec::new() }
    }

    fn len(&self) -> usize {
        self.bits.len()
    }

    fn push(&mut self, value: bool) {
        self.bits.push(value);
    }

    fn add_bits(&mut self, value: impl ToBits, bits: u8) -> Result<(), ()> {
        self.bits.extend_from_slice(&value.to_bits(bits)?);
        Ok(())
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut byte = 0;
        let mut byte_index = 0;
        for &bit in self.bits.iter() {
            if byte_index == 8 {
                bytes.push(byte);
                byte = 0;
                byte_index = 0;
            }
            if bit {
                byte |= 1 << byte_index;
            }
            byte_index += 1;
        }
        bytes
    }
}

trait ToBits {
    fn to_bits(&self, bits: u8) -> Result<Vec<bool>, ()>;
}

impl ToBits for u8 {
    fn to_bits(&self, bits: u8) -> Result<Vec<bool>, ()> {
        if 8 - self.leading_zeros() > bits as u32 {
            return Err(()); // too large to fit
        }
        let mut b = Vec::new();
        for i in 0..bits {
            b.push((self & (1 << i)) != 0);
        }
        Ok(b)
    }
}

impl ToBits for u16 {
    fn to_bits(&self, bits: u8) -> Result<Vec<bool>, ()> {
        if 16 - self.leading_zeros() > bits as u32 {
            return Err(()); // too large to fit
        }
        let mut b = Vec::new();
        for i in 0..bits {
            b.push((self & (1 << i)) != 0);
        }
        Ok(b)
    }
}

impl ToBits for u32 {
    fn to_bits(&self, bits: u8) -> Result<Vec<bool>, ()> {
        if 32 - self.leading_zeros() > bits as u32 {
            return Err(()); // too large to fit
        }
        let mut b = Vec::new();
        for i in 0..bits {
            b.push((self & (1 << i)) != 0);
        }
        Ok(b)
    }
}

impl ToBits for i32 {
    fn to_bits(&self, bits: u8) -> Result<Vec<bool>, ()> {
        if *self >= 0 {
            if 32 - self.leading_zeros() >= bits as u32 {
                return Err(()); // too large to fit
            }
        } else {
            if 32 - self.leading_ones() >= bits as u32 {
                return Err(()); // too large to fit
            }
        }
        let mut b = Vec::new();
        for i in 0..bits {
            b.push((self & (1 << i)) != 0);
        }
        Ok(b)
    }
}

#[cfg(test)]
mod tests {
    use embedded_graphics::{mock_display::MockDisplay, pixelcolor::BinaryColor, prelude::Point};
    use u8g2_fonts::{
        types::{FontColor, VerticalPosition},
        Font, FontRenderer,
    };

    use super::*;

    #[test]
    fn test_to_bits() {
        assert_eq!(
            32u8.to_bits(8),
            Ok(vec![false, false, false, false, false, true, false, false])
        );
        assert_eq!((4i32).to_bits(3), Err(()));
        assert_eq!((3i32).to_bits(3), Ok(vec![true, true, false]));
        assert_eq!((-4i32).to_bits(3), Ok(vec![false, false, true]));
        assert_eq!((-5i32).to_bits(3), Err(()));
    }

    struct File;
    impl Font for File {
        const DATA: &'static [u8] = include_bytes!("../wenquanyi_12pt.u8g2font");
    }

    #[test]
    fn test_file() {
        let font = FontRenderer::new::<File>();

        let text = "g";
        let pos = Point::new(8, 0);
        let vertical_pos = VerticalPosition::Baseline;

        println!(
            "{:?}",
            font.get_rendered_dimensions(text, pos, vertical_pos)
                .unwrap()
        );

        let mut mock_display = MockDisplay::new();
        font.render(
            text,
            pos,
            vertical_pos,
            FontColor::Transparent(BinaryColor::On),
            &mut mock_display,
        )
        .unwrap();

        println!("{:?}", mock_display);
    }

    // #[test]
    // fn test_font_rendering() {
    //     let mut display: SimulatorDisplay<BinaryColor> = SimulatorDisplay::new(Size::new(128, 64));

    // }
}

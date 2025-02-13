use bitstream_io::{BitWrite, BitWriter, LittleEndian};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

pub struct U8g2File {
    pub header: U8g2Header,
    pub short_glyphs: Vec<U8g2Glyph>,
    pub long_glyphs: Vec<U8g2Glyph>,
}

#[allow(dead_code)] // some fields are not read but are needed for the layout
#[derive(Debug, Clone, IntoBytes, FromBytes, KnownLayout, Immutable)]
#[repr(packed)]
pub struct U8g2Header {
    pub glyph_cnt: u8,
    pub bbx_mode: u8,
    pub bits_per_0: u8,
    pub bits_per_1: u8,
    pub bits_per_char_width: u8,
    pub bits_per_char_height: u8,
    pub bits_per_char_x: u8,
    pub bits_per_char_y: u8,
    pub bits_per_advance: u8,
    pub max_char_width: i8,
    pub max_char_height: i8,
    pub x_offset: i8,
    pub y_offset: i8,
    pub ascent_a: i8,
    pub descent_g: i8,
    pub ascent_para: i8,
    pub descent_para: i8,
    pub start_pos_upper_a: u16,
    pub start_pos_lower_a: u16,
    pub start_pos_unicode: u16,
}

const U8G2_HEADER_SIZE: usize = std::mem::size_of::<U8g2Header>();

impl U8g2File {
    pub fn to_bytes(&mut self) -> Vec<u8> {
        // glyphs
        let mut bytes = Vec::new();

        // short glyphs
        for i in 0..self.short_glyphs.len() {
            let glyph = &self.short_glyphs[i];

            let no_jump = i == self.short_glyphs.len() - 1;
            bytes.extend_from_slice(&glyph.to_bytes(&self.header, no_jump));
        }

        let long_offset = if !self.long_glyphs.is_empty() {
            let long_offset = bytes.len();

            // jump table
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

            Some(long_offset)
        } else {
            None
        };

        // serves as test that font file is searchable
        print!("checking font file, found: ");
        let mut offset = 0;
        let mut upper_a = None;
        let mut lower_a = None;
        // check for all short glyphs
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
        // check for all long glyphs
        if let Some(long_offset) = long_offset {
            offset = long_offset + 4; // skip jump table
            loop {
                let codepoint = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]);
                print!("{}", char::from_u32(codepoint as u32).unwrap());

                let jump = bytes[offset + 2];
                if jump == 0 {
                    break;
                }

                offset += jump as usize;
            }
        }
        println!();

        // change header glyph indices to byte indices
        self.header.glyph_cnt = (self.short_glyphs.len() + self.long_glyphs.len()) as u8;
        self.header.start_pos_upper_a = upper_a.unwrap().try_into().unwrap();
        self.header.start_pos_lower_a = lower_a.unwrap().try_into().unwrap();
        self.header.start_pos_unicode = long_offset.unwrap_or(0).try_into().unwrap();

        // make two byte header variables big-endian *byte* order TODO: why?
        self.header.start_pos_upper_a = self.header.start_pos_upper_a.swap_bytes();
        self.header.start_pos_lower_a = self.header.start_pos_lower_a.swap_bytes();
        self.header.start_pos_unicode = self.header.start_pos_unicode.swap_bytes();

        // insert header
        let h = self.header.as_bytes();
        assert!(U8G2_HEADER_SIZE == 23);
        assert!(U8G2_HEADER_SIZE == h.len());
        bytes.splice(0..0, h.iter().copied());

        bytes
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Unicode {
    Single(u8),
    Double(u16),
}

#[derive(Debug)]
pub struct U8g2Glyph {
    pub unicode: Unicode,
    pub width: u32,
    pub height: u32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub advance: i32,
    pub bitmap: Vec<((u32, u32), u32)>,
}

impl U8g2Glyph {
    pub fn to_bytes(&self, header: &U8g2Header, no_jump: bool) -> Vec<u8> {
        let mut bits: BitWriter<Vec<u8>, LittleEndian> = BitWriter::new(Vec::new());

        // unicode
        let jump_pos = match self.unicode {
            Unicode::Single(c) => {
                bits.write(8, c).unwrap();
                1
            }
            Unicode::Double(c) => {
                // make two byte unicode big-endian *byte* order TODO: why?
                bits.write(16, c.swap_bytes()).unwrap();
                2
            }
        };

        // jump
        bits.write(8, 0).unwrap();

        // rest of header
        // for some reason the format doesn't use two's complement and need to be converted
        fn sign(bits: u32, v: i32) -> u32 {
            v as u32 + (1 << (bits - 1))
        }
        let bpcw = header.bits_per_char_width.into();
        let bpch = header.bits_per_char_height.into();
        let bpcx = header.bits_per_char_x.into();
        let bpcy = header.bits_per_char_y.into();
        let bpadv = header.bits_per_advance.into();
        bits.write(bpcw, self.width).unwrap();
        bits.write(bpch, self.height).unwrap();
        bits.write(bpcx, sign(bpcx, self.offset_x)).unwrap();
        bits.write(bpcy, sign(bpcy, self.offset_y)).unwrap();
        bits.write(bpadv, sign(bpadv, self.advance)).unwrap();

        // bitmap
        for &((zeros, ones), repeat) in self.bitmap.iter() {
            bits.write(header.bits_per_0.into(), zeros).unwrap();
            bits.write(header.bits_per_1.into(), ones).unwrap();
            for _ in 0..repeat {
                bits.write_bit(true).unwrap();
            }
            bits.write_bit(false).unwrap();
        }

        // make sure no partial bytes are left
        bits.write(7, 0).unwrap();
        let mut bytes = bits.into_writer();

        // update jump
        if !no_jump {
            bytes[jump_pos] = bytes.len().try_into().unwrap();
        }

        bytes
    }
}

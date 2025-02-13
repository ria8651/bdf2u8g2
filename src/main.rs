use clap::{Arg, ArgAction, Command};
use std::{collections::HashSet, fs, path::PathBuf};
use u8g2_file::{U8g2File, U8g2Glyph, U8g2Header, Unicode};

#[cfg(test)]
mod tests;
mod u8g2_file;

fn main() {
    let app = Command::new("convert-bdf")
         .arg_required_else_help(true)
        .about("Generate u8g2 files from bdf font files. If no range is specified, all glyphs will be exported. If multiple range arguments are specified, they will be combined.")
        .arg(
            Arg::new("input")
                .long("bdf-file")
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
                .help("Output file for the u8g2 font")
                .short('o')
                .value_parser(clap::value_parser!(PathBuf))
                .default_value("./")
                .action(ArgAction::Set)
                .value_name("PATH"),
        )
        .arg(
            Arg::new("range")
                .long("range")
                .help("Characters to put in the font file e.g --range \"abc\" means only export the a,b and c glyphs.")
                .action(ArgAction::Append)
                .value_name("RANGE"),
        )
        .arg(
            Arg::new("range-file")
                .long("range-file")
                .help(
                   r#"same as range option, but uses all the characters in a file."#
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

    let mut short_glyphs: Vec<U8g2Glyph> = Vec::new();
    let mut long_glyphs: Vec<U8g2Glyph> = Vec::new();
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
        let offset_x = bb.offset.x;
        let offset_y = bb.offset.y;
        // let advance = width as i32;
        let advance = glyph.device_width.x;
        // let offset_x = 0;
        // let offset_y = 0;
        // let advance = 0;
        // println!("{}: {:?}", c, advance);

        // bitmap
        let mut length_pairs: Vec<(u32, u32)> = Vec::new();
        let mut run_0 = 0;
        let mut run_1 = 0;
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

        let glyph = U8g2Glyph {
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

    // check for missing characters
    if !hash_char_set.is_empty() {
        print!("missing characters: ");
        for c in hash_char_set.iter() {
            print!("{}", c);
        }
        println!();
    }

    // header
    let header = U8g2Header {
        // seems kind of useless as it only goes up to 255 and isn't used in the parser
        glyph_cnt: 0,
        //
        bbx_mode: 0,
        bits_per_0: run_bits_0,
        bits_per_1: run_bits_1,
        bits_per_char_width: 5,
        bits_per_char_height: 5,
        bits_per_char_x: 4,
        bits_per_char_y: 5,
        bits_per_advance: 6,
        max_char_width: font.metadata.bounding_box.size.x.try_into().unwrap(),
        max_char_height: font.metadata.bounding_box.size.y.try_into().unwrap(),
        x_offset: font.metadata.bounding_box.offset.x.try_into().unwrap(),
        y_offset: font.metadata.bounding_box.offset.y.try_into().unwrap(),
        // not sure why these are needed when the info is in each glyph? seems to work fine without them
        ascent_a: 0,
        descent_g: 0,
        ascent_para: 0,
        descent_para: 0,
        //
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

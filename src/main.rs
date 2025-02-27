use bdf_parser::Property;
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
                .long("input")
                .help("Input bdf file or folder of bdf files")
                .short('i')
                .value_parser(clap::value_parser!(PathBuf))
                .required(true)
                .action(ArgAction::Set)
                .value_name("FILE"),
        )
        .arg(
            Arg::new("output")
                .long("output")
                .help("Output file or folder for the u8g2 font. If input is a folder, this must be a folder. Defaults to the input file with the extension changed to u8g2font.")
                .short('o')
                .value_parser(clap::value_parser!(PathBuf))
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

    // get char set
    let mut char_set = Vec::<char>::new();
    if let Some(ss) = matches.get_many::<String>("range") {
        for p in ss.collect::<Vec<_>>() {
            char_set.extend(p.chars());
        }
    }
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
    char_set.sort();
    char_set.dedup();

    // convert based on input type
    let input = matches.get_one::<PathBuf>("input").unwrap();
    let output = matches.get_one::<PathBuf>("output");
    if input.is_dir() {
        if output.is_some_and(|p| !p.is_dir()) {
            panic!("output must be a directory if input is a directory");
        }

        for entry in fs::read_dir(input).expect("couldn't open input directory") {
            let entry = entry.expect("couldn't read entry");
            let path = entry.path();
            if path.extension().unwrap() == "bdf" {
                let mut output = output.cloned().unwrap_or(input.clone());
                output.push(path.file_stem().unwrap());
                output.set_extension("u8g2font");
                convert_bdf_to_u8g2(&path, &output, &char_set);
            }
        }
    }
    if input.is_file() {
        let output = output.cloned().unwrap_or_else(|| {
            let mut output = input.clone();
            output.set_extension("u8g2font");
            output
        });
        convert_bdf_to_u8g2(input, &output, &char_set);
    }
}

fn convert_bdf_to_u8g2(input: &PathBuf, output: &PathBuf, char_set: &[char]) {
    let bdf = fs::read(input).expect("couldn't open BDF file");
    let font = bdf_parser::BdfFont::parse(&bdf).expect("BDF file is bad format");

    println!(
        "converting {} to {}",
        input.file_name().unwrap().to_str().unwrap(),
        output.file_name().unwrap().to_str().unwrap()
    );

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
        let advance = glyph.device_width.x;

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
        print!("missing glyphs: ");
        for c in hash_char_set.iter() {
            print!("{}", c);
        }
        println!();
    }

    // header
    let ascent_a = short_glyphs
        .iter()
        .find(|g| matches!(g.unicode, Unicode::Single(b'A')))
        .map(|g| g.height as i32 + g.offset_y)
        .or_else(|| font.properties.try_get(Property::FontAscent).ok())
        .expect("couldn't find ascent_a");
    let descent_g = short_glyphs
        .iter()
        .find(|g| matches!(g.unicode, Unicode::Single(b'g')))
        .map(|g| -g.offset_y)
        .or_else(|| font.properties.try_get(Property::FontDescent).ok())
        .expect("couldn't find descent_g");
    let header = U8g2Header {
        // seems kind of useless as it only goes up to 255 and isn't used in the parser
        glyph_cnt: 0,
        //
        bbx_mode: 1,
        bits_per_0: run_bits_0,
        bits_per_1: run_bits_1,
        bits_per_char_width: 5,
        bits_per_char_height: 5,
        bits_per_char_x: 4,
        bits_per_char_y: 5,
        bits_per_advance: 7,
        max_char_width: font.metadata.bounding_box.size.x.try_into().unwrap(),
        max_char_height: font.metadata.bounding_box.size.y.try_into().unwrap(),
        x_offset: font.metadata.bounding_box.offset.x.try_into().unwrap(),
        y_offset: font.metadata.bounding_box.offset.y.try_into().unwrap(),
        ascent_a: ascent_a.try_into().unwrap(),
        descent_g: descent_g.try_into().unwrap(),
        // not sure why these are needed when the info is in each glyph? seems to work fine without them
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

# BDF to u8g2 font converter

Lets you convert BDF fonts to u8g2 fonts using a subset of the characters in the font. Made for use with [u8g2-fonts](https://github.com/Finomnis/u8g2-fonts) to reduce the size of the fonts. I was originally using [embedded-fonts](https://github.com/flyingyizi/embedded-fonts) by flyingyizi. To achieve smaller font sizes and more advanced font rendering I modified the font converter to create u8g2 fonts instead of a custom format.

## Usage

See `bdf2u8g2 --help` for more information.

```txt
bdf2u8g2 [OPTIONS] --bdf-file <FILE>
```

## Todo

- [ ] Find the correct bit widths for the characters. At the momement the bit widths are hardcoded and panics if a value too large is found in the bdf file.
- [ ] Optimize the RLE encoding by trying different bit widths
- [ ] Add customization of the u8g2 font header
- [ ] Less unwraps and panics

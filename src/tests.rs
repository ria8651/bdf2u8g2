use embedded_graphics::{mock_display::MockDisplay, pixelcolor::BinaryColor, prelude::Point};
use u8g2_fonts::{
    types::{FontColor, VerticalPosition},
    Font, FontRenderer,
};

struct File;
impl Font for File {
    const DATA: &'static [u8] = include_bytes!("../icons.u8g2font");
}

#[test]
fn test_file() {
    let font = FontRenderer::new::<File>();

    let text = "!";
    let pos = Point::new(0, 24);
    let vertical_pos = VerticalPosition::Top;

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

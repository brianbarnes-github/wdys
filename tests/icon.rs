use wdys::icon::with_status_dot;

const SIZE: u32 = 32;
const BASE_PIXEL: [u8; 4] = [10, 20, 30, 255];

fn base() -> Vec<u8> {
    BASE_PIXEL.repeat((SIZE * SIZE) as usize)
}

fn pixel(rgba: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * SIZE + x) * 4) as usize;
    [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
}

#[test]
fn no_status_returns_the_base_icon_unchanged() {
    assert_eq!(with_status_dot(&base(), SIZE, None), base());
}

#[test]
fn status_keeps_the_image_size() {
    assert_eq!(
        with_status_dot(&base(), SIZE, Some([255, 0, 0])).len(),
        base().len()
    );
}

#[test]
fn dot_centre_is_the_status_colour_in_the_bottom_right() {
    let icon = with_status_dot(&base(), SIZE, Some([235, 40, 40]));
    assert_eq!(pixel(&icon, 24, 24), [235, 40, 40, 255]);
}

#[test]
fn dot_has_a_white_ring_so_it_stands_out() {
    let icon = with_status_dot(&base(), SIZE, Some([235, 40, 40]));
    assert_eq!(pixel(&icon, 30, 24), [255, 255, 255, 255]);
}

#[test]
fn rest_of_the_icon_is_untouched() {
    let icon = with_status_dot(&base(), SIZE, Some([235, 40, 40]));
    assert_eq!(pixel(&icon, 0, 0), BASE_PIXEL);
    assert_eq!(pixel(&icon, 5, 28), BASE_PIXEL);
}

#[test]
fn bundled_tray_artwork_is_a_32x32_rgba_image() {
    assert_eq!(
        include_bytes!("../resources/tray-32.rgba").len(),
        (SIZE * SIZE * 4) as usize
    );
}

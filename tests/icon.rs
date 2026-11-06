use wdys::icon::circle_icon_rgba;

fn pixel(rgba: &[u8], size: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * size + x) * 4) as usize;
    [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
}

#[test]
fn icon_has_rgba_length_for_its_size() {
    assert_eq!(circle_icon_rgba(16, [1, 2, 3]).len(), 16 * 16 * 4);
}

#[test]
fn centre_is_opaque_in_the_requested_colour() {
    let icon = circle_icon_rgba(16, [200, 30, 40]);
    assert_eq!(pixel(&icon, 16, 8, 8), [200, 30, 40, 255]);
}

#[test]
fn corners_are_transparent() {
    let icon = circle_icon_rgba(16, [200, 30, 40]);
    assert_eq!(pixel(&icon, 16, 0, 0)[3], 0);
    assert_eq!(pixel(&icon, 16, 15, 15)[3], 0);
}

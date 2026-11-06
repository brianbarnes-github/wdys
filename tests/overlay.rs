use wdys::overlay::{overlay_frame, OverlayFrame, OVERLAY_HEIGHT, OVERLAY_WIDTH};

const ICON_SIZE: u32 = 32;

fn green_icon() -> Vec<u8> {
    [0u8, 255, 0, 255].repeat((ICON_SIZE * ICON_SIZE) as usize)
}

fn px(frame: &OverlayFrame, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * frame.width + x) * 4) as usize;
    [frame.bgra[i], frame.bgra[i + 1], frame.bgra[i + 2], frame.bgra[i + 3]]
}

#[test]
fn frame_has_the_documented_size() {
    let frame = overlay_frame(&green_icon(), ICON_SIZE, true);
    assert_eq!((frame.width, frame.height), (OVERLAY_WIDTH, OVERLAY_HEIGHT));
    assert_eq!(frame.bgra.len(), (OVERLAY_WIDTH * OVERLAY_HEIGHT * 4) as usize);
}

#[test]
fn corners_are_transparent_so_the_pill_is_rounded() {
    let frame = overlay_frame(&green_icon(), ICON_SIZE, true);
    assert_eq!(px(&frame, 0, 0)[3], 0);
    assert_eq!(px(&frame, OVERLAY_WIDTH - 1, OVERLAY_HEIGHT - 1)[3], 0);
}

#[test]
fn pill_interior_is_a_translucent_dark_background() {
    let frame = overlay_frame(&green_icon(), ICON_SIZE, true);
    assert_eq!(px(&frame, 5, 22)[3], 220);
}

#[test]
fn bright_dot_is_opaque_red() {
    let frame = overlay_frame(&green_icon(), ICON_SIZE, true);
    assert_eq!(px(&frame, 20, 22), [40, 40, 235, 255]); // BGRA
}

#[test]
fn dim_dot_differs_from_bright_dot_so_it_can_pulse() {
    let bright = overlay_frame(&green_icon(), ICON_SIZE, true);
    let dim = overlay_frame(&green_icon(), ICON_SIZE, false);
    assert_ne!(px(&bright, 20, 22), px(&dim, 20, 22));
    assert_eq!(px(&dim, 20, 22)[3], 255);
}

#[test]
fn app_icon_is_drawn_to_the_right_of_the_dot() {
    let frame = overlay_frame(&green_icon(), ICON_SIZE, true);
    assert_eq!(px(&frame, 54, 22), [0, 255, 0, 255]);
}

#[test]
fn transparent_icon_pixels_show_the_pill_underneath() {
    let clear_icon = [0u8, 0, 0, 0].repeat((ICON_SIZE * ICON_SIZE) as usize);
    let frame = overlay_frame(&clear_icon, ICON_SIZE, true);
    assert_eq!(px(&frame, 54, 22)[3], 220);
}

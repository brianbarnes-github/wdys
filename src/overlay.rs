pub const OVERLAY_WIDTH: u32 = 82;
pub const OVERLAY_HEIGHT: u32 = 44;

const PILL_RGB: [f32; 3] = [24.0, 24.0, 28.0];
const PILL_ALPHA: f32 = 220.0 / 255.0;
const DOT_CENTRE: (f32, f32) = (20.0, 22.0);
const DOT_RADIUS: f32 = 8.0;
const DOT_BRIGHT: [f32; 3] = [235.0, 40.0, 40.0];
const DOT_DIM: [f32; 3] = [120.0, 30.0, 30.0];
const ICON_ORIGIN: (u32, u32) = (38, 6);

/// A rendered overlay image: 32-bit BGRA with premultiplied alpha, rows top to bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayFrame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

/// Premultiplied RGBA with channels in 0.0..=1.0.
type Pixel = [f32; 4];

/// Composites a colour (0..=255 channels) with the given opacity over `dst`.
fn over(dst: Pixel, rgb: [f32; 3], alpha: f32) -> Pixel {
    let keep = 1.0 - alpha;
    [
        rgb[0] / 255.0 * alpha + dst[0] * keep,
        rgb[1] / 255.0 * alpha + dst[1] * keep,
        rgb[2] / 255.0 * alpha + dst[2] * keep,
        alpha + dst[3] * keep,
    ]
}

/// Anti-aliased coverage (0..=1) of a point inside a shape given its signed distance.
fn coverage(distance_outside: f32) -> f32 {
    (0.5 - distance_outside).clamp(0.0, 1.0)
}

/// Distance from the pixel centre to the pill's outline (negative inside); the pill is a
/// capsule: a segment along the middle, thickened by half the height.
fn pill_distance(x: f32, y: f32) -> f32 {
    let radius = OVERLAY_HEIGHT as f32 / 2.0;
    let nearest_x = x.clamp(radius, OVERLAY_WIDTH as f32 - radius);
    ((x - nearest_x).powi(2) + (y - radius).powi(2)).sqrt() - radius
}

fn icon_pixel(icon: &[u8], icon_size: u32, x: u32, y: u32) -> Option<([f32; 3], f32)> {
    let (ox, oy) = ICON_ORIGIN;
    let (ix, iy) = (x.checked_sub(ox)?, y.checked_sub(oy)?);
    if ix >= icon_size || iy >= icon_size {
        return None;
    }
    let i = ((iy * icon_size + ix) * 4) as usize;
    let px = icon.get(i..i + 4)?;
    Some((
        [f32::from(px[0]), f32::from(px[1]), f32::from(px[2])],
        f32::from(px[3]) / 255.0,
    ))
}

/// Draws the hands-free indicator: a dark pill holding a red dot (bright or dim, so it can
/// pulse) and the app icon (`icon_size` square RGBA).
pub fn overlay_frame(icon_rgba: &[u8], icon_size: u32, bright: bool) -> OverlayFrame {
    let dot_colour = if bright { DOT_BRIGHT } else { DOT_DIM };
    let bgra = (0..OVERLAY_HEIGHT)
        .flat_map(|y| (0..OVERLAY_WIDTH).map(move |x| (x, y)))
        .flat_map(|(x, y)| {
            let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
            let mut pixel: Pixel = [0.0; 4];
            pixel = over(
                pixel,
                PILL_RGB,
                PILL_ALPHA * coverage(pill_distance(cx, cy)),
            );
            let dot_distance =
                ((cx - DOT_CENTRE.0).powi(2) + (cy - DOT_CENTRE.1).powi(2)).sqrt() - DOT_RADIUS;
            pixel = over(pixel, dot_colour, coverage(dot_distance));
            if let Some((rgb, alpha)) = icon_pixel(icon_rgba, icon_size, x, y) {
                pixel = over(pixel, rgb, alpha);
            }
            let to_byte = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
            [
                to_byte(pixel[2]),
                to_byte(pixel[1]),
                to_byte(pixel[0]),
                to_byte(pixel[3]),
            ]
        })
        .collect();
    OverlayFrame {
        width: OVERLAY_WIDTH,
        height: OVERLAY_HEIGHT,
        bgra,
    }
}

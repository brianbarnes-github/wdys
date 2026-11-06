const OUTER_RADIUS_FRACTION: f32 = 0.22;
const RING_WIDTH_FRACTION: f32 = 0.0625;

/// Returns a copy of `base` (square RGBA, `size` pixels a side) with a small status dot,
/// ringed in white, in the bottom-right corner. `None` returns the base icon unchanged.
pub fn with_status_dot(base: &[u8], size: u32, rgb: Option<[u8; 3]>) -> Vec<u8> {
    let Some(rgb) = rgb else {
        return base.to_vec();
    };
    let outer = (size as f32 * OUTER_RADIUS_FRACTION).round();
    let ring = (size as f32 * RING_WIDTH_FRACTION).round().max(1.0);
    let centre = size as f32 - outer - 1.0;
    base.as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .flat_map(|(i, px)| {
            let (x, y) = ((i as u32 % size) as f32, (i as u32 / size) as f32);
            let distance = ((x - centre).powi(2) + (y - centre).powi(2)).sqrt();
            if distance > outer {
                *px
            } else if distance > outer - ring {
                [255, 255, 255, 255]
            } else {
                [rgb[0], rgb[1], rgb[2], 255]
            }
        })
        .collect()
}

/// Square RGBA image of a filled circle on a transparent background, for tray icons.
pub fn circle_icon_rgba(size: u32, rgb: [u8; 3]) -> Vec<u8> {
    let centre = (size as f32 - 1.0) / 2.0;
    let radius = size as f32 / 2.0 - 1.0;
    (0..size)
        .flat_map(|y| (0..size).map(move |x| (x, y)))
        .flat_map(|(x, y)| {
            let (dx, dy) = (x as f32 - centre, y as f32 - centre);
            let inside = (dx * dx + dy * dy).sqrt() <= radius;
            [rgb[0], rgb[1], rgb[2], if inside { 255 } else { 0 }]
        })
        .collect()
}

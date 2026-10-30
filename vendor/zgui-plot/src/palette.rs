//! Distinct automatic series colors.
// Colors spread through OKLab space, with at least 5.5:1 contrast on plot background.
pub const DEFAULT_PALETTE: [[u8; 4]; 16] = [
    [78, 180, 255, 255],
    [255, 165, 0, 255],
    [255, 0, 240, 255],
    [0, 255, 0, 255],
    [15, 165, 60, 255],
    [165, 255, 240, 255],
    [255, 165, 240, 255],
    [255, 75, 105, 255],
    [255, 240, 0, 255],
    [180, 120, 255, 255],
    [135, 210, 120, 255],
    [180, 135, 75, 255],
    [15, 165, 165, 255],
    [195, 120, 165, 255],
    [0, 225, 225, 255],
    [255, 210, 150, 255],
];

/// Prefer unused destination colors, then least-used colors across workspace.
pub fn next_color(local: &[[u8; 4]], global: &[[u8; 4]]) -> [u8; 4] {
    *DEFAULT_PALETTE
        .iter()
        .min_by_key(|color| {
            (
                local.iter().filter(|used| used[..3] == color[..3]).count(),
                global.iter().filter(|used| used[..3] == color[..3]).count(),
            )
        })
        .unwrap()
}

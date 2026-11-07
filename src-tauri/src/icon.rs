//! Tray tile: the 5h-window cost as crisp pixel digits on a rounded juice-orange square.
use tauri::image::Image;

const ORANGE: [u8; 3] = [0xFF, 0x8A, 0x1F];
const GREY: [u8; 3] = [0x8B, 0x8B, 0x8B];
const INK: [u8; 3] = [0x1B, 0x1B, 0x1B];

/// 5-row pixel glyph: (width in columns, rows as bit masks with the leftmost column as the highest bit).
fn glyph(c: char) -> (u32, [u8; 5]) {
    match c {
        '0' => (3, [7, 5, 5, 5, 7]),
        '1' => (3, [2, 6, 2, 2, 7]),
        '2' => (3, [7, 1, 7, 4, 7]),
        '3' => (3, [7, 1, 3, 1, 7]),
        '4' => (3, [5, 5, 7, 1, 1]),
        '5' => (3, [7, 4, 7, 1, 7]),
        '6' => (3, [7, 4, 7, 5, 7]),
        '7' => (3, [7, 1, 2, 2, 2]),
        '8' => (3, [7, 5, 7, 5, 7]),
        '9' => (3, [7, 5, 7, 1, 7]),
        'k' => (3, [4, 5, 6, 5, 5]),
        '.' => (1, [0, 0, 0, 0, 1]),
        _ => unreachable!("icon glyph {c:?} not in label alphabet"),
    }
}

/// Short tray label for a dollar amount: ".4", "42", "999", "3k" (at most 3 glyphs).
pub fn label(cost: f64) -> String {
    if cost < 0.95 {
        format!(".{}", (cost * 10.0).round() as u32)
    } else if cost < 999.5 {
        format!("{}", cost.round() as u32)
    } else {
        format!("{}k", ((cost / 1000.0).round() as u32).min(99))
    }
}

/// Tray image for a 5h cost at the shell's current small-icon size.
pub fn tile(cost: f64, active: bool) -> Image<'static> {
    let size = tray_size();
    Image::new_owned(render(&label(cost), active, size), size, size)
}

/// RGBA pixels of a `size`×`size` tile showing `text`; grey when no 5h window is active.
pub fn render(text: &str, active: bool, size: u32) -> Vec<u8> {
    let bg = if active { ORANGE } else { GREY };
    let n = size as f32;
    let r = n / 5.0;
    let mut px = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let a = cover((x as f32 + 0.5, y as f32 + 0.5), (0.0, 0.0, n, n), r);
            let i = ((y * size + x) * 4) as usize;
            px[i..i + 3].copy_from_slice(&bg);
            px[i + 3] = (a * 255.0) as u8;
        }
    }
    draw_text(&mut px, size, text);
    px
}

/// How much of the pixel centered at `p` a rounded rectangle `(x0, y0, x1, y1)` with corner radius `r` covers (0..1).
/// The distance into the nearest corner gives antialiased edges; `r` = half the side draws a circle.
pub fn cover((fx, fy): (f32, f32), (x0, y0, x1, y1): (f32, f32, f32, f32), r: f32) -> f32 {
    let cx = fx.clamp(x0 + r, x1 - r);
    let cy = fy.clamp(y0 + r, y1 - r);
    let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
    (r + 0.5 - d.max(r - 0.5)).clamp(0.0, 1.0)
}

/// Paints `text` centered in whole-pixel cells: never wider than tall, at most one pixel taller than wide.
fn draw_text(px: &mut [u8], size: u32, text: &str) {
    let glyphs: Vec<_> = text.chars().map(glyph).collect();
    let cols = glyphs.iter().map(|g| g.0).sum::<u32>() + glyphs.len().saturating_sub(1) as u32;
    let room = size - 2 * (size / 16).max(1);
    let kx = (room / cols.max(1)).min(room / 5).max(1);
    let ky = (room / 5).min(kx + 1).max(1);
    let (mut ox, oy) = (size.saturating_sub(cols * kx) / 2, size.saturating_sub(5 * ky) / 2);
    for (w, rows) in glyphs {
        for (ry, bits) in rows.iter().enumerate() {
            for cx in 0..w {
                if bits >> (w - 1 - cx) & 1 == 1 {
                    fill(px, size, (ox + cx * kx, oy + ry as u32 * ky), (kx, ky));
                }
            }
        }
        ox += (w + 1) * kx;
    }
}

/// Fills a kx×ky ink cell at (x, y), clipped to the tile.
fn fill(px: &mut [u8], size: u32, (x, y): (u32, u32), (kx, ky): (u32, u32)) {
    for yy in y..(y + ky).min(size) {
        for xx in x..(x + kx).min(size) {
            let i = ((yy * size + xx) * 4) as usize;
            px[i..i + 3].copy_from_slice(&INK);
            px[i + 3] = 255;
        }
    }
}

/// Small-icon size the shell uses for tray icons at the current DPI.
#[cfg(windows)]
pub fn tray_size() -> u32 {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSMICON};
    // SAFETY: GetSystemMetrics has no preconditions.
    let s = unsafe { GetSystemMetrics(SM_CXSMICON) };
    u32::try_from(s).ok().filter(|s| *s >= 16).unwrap_or(16)
}

#[cfg(not(windows))]
pub fn tray_size() -> u32 {
    32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_label_buckets() {
        assert_eq!(label(0.0), ".0");
        assert_eq!(label(0.42), ".4");
        assert_eq!(label(0.96), "1");
        assert_eq!(label(41.6), "42");
        assert_eq!(label(999.4), "999");
        assert_eq!(label(3210.0), "3k");
        assert_eq!(label(1e9), "99k");
        let px = render(&label(41.6), true, 16);
        assert_eq!(px.len(), 16 * 16 * 4);
        assert!(px.chunks(4).any(|p| p[..3] == INK));
    }
}

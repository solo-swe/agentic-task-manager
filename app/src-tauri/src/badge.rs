//! The app's badge (T-518, docs/app-desktop-ui-design.md §7): the workspace's needs-you count (in review, blocked on
//! the user) on the app's icon, which Settings turns off (T-517). macOS and Linux show a count (`set_badge_count`;
//! Linux where the desktop shows one); Windows, where Tauri has no count, a small icon of it over the taskbar button
//! (`set_overlay_icon`).
// The overlay icon is Windows' only; its tests run everywhere
#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use tauri::{Runtime, WebviewWindow};

use crate::commands::AppError;

/// The overlay icon's size, in pixels (Windows draws it at 16×16).
const SIZE: u32 = 16;
/// The badge's colour and its text's (RGBA).
const RED: [u8; 4] = [0xd9, 0x30, 0x25, 0xff];
const WHITE: [u8; 4] = [0xff, 0xff, 0xff, 0xff];

/// The count a badge shows: none for nothing (no badge).
#[must_use]
pub fn badge_count(count: Option<u32>) -> Option<i64> {
    count.filter(|n| *n > 0).map(i64::from)
}

/// A 3×5 glyph per character the overlay icon writes, a row per entry, its three bits left to right.
fn glyph(c: char) -> [u8; 5] {
    match c {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b001, 0b001, 0b001],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        _ => [0b000, 0b010, 0b111, 0b010, 0b000],
    }
}

/// Windows' overlay icon for `count` (at least 1): a red disc with the count in white, "9+" past nine. Its RGBA
/// pixels, `SIZE` × `SIZE`.
#[must_use]
pub fn overlay_icon(count: u32) -> Vec<u8> {
    let size = SIZE as usize;
    let mut pixels = vec![0u8; size * size * 4];
    let mut put = |x: usize, y: usize, colour: [u8; 4]| {
        if x < size && y < size {
            let at = (y * size + x) * 4;
            pixels[at..at + 4].copy_from_slice(&colour);
        }
    };
    // The disc, in half pixels from its centre (each pixel's middle), so it stays whole numbers
    let diameter = i64::from(SIZE);
    for (y, row) in (0..diameter).zip(0..size) {
        for (x, col) in (0..diameter).zip(0..size) {
            let (dx, dy) = (2 * x + 1 - diameter, 2 * y + 1 - diameter);
            if dx * dx + dy * dy <= diameter * diameter {
                put(col, row, RED);
            }
        }
    }
    // The count: one digit twice the glyphs' size, two characters at their size
    let text = if count > 9 {
        "9+".to_owned()
    } else {
        count.to_string()
    };
    let (scale, gap) = if text.len() == 1 { (2, 0) } else { (1, 1) };
    let width = text.len() * 3 * scale + (text.len() - 1) * gap;
    let (left, top) = ((size - width) / 2, (size - 5 * scale) / 2);
    for (n, c) in text.chars().enumerate() {
        let x0 = left + n * (3 * scale + gap);
        for (row, bits) in glyph(c).iter().enumerate() {
            for col in 0..3 {
                if bits & (0b100 >> col) != 0 {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            put(x0 + col * scale + dx, top + row * scale + dy, WHITE);
                        }
                    }
                }
            }
        }
    }
    pixels
}

/// Show `count` (none, or 0, for no badge) on the app's icon, from the calling window (the count is the
/// workspace's, the same from every window).
///
/// # Errors
/// When the platform can't show it.
#[tauri::command]
#[specta::specta]
pub async fn set_badge<R: Runtime>(
    window: WebviewWindow<R>,
    count: Option<u32>,
) -> Result<(), AppError> {
    #[cfg(target_os = "windows")]
    let shown = window.set_overlay_icon(
        badge_count(count)
            .and(count)
            .map(|n| tauri::image::Image::new_owned(overlay_icon(n), SIZE, SIZE)),
    );
    #[cfg(not(target_os = "windows"))]
    let shown = window.set_badge_count(badge_count(count));
    shown.map_err(|e| AppError::Failed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_waiting_is_no_badge() {
        assert_eq!(badge_count(None), None);
        assert_eq!(badge_count(Some(0)), None);
        assert_eq!(badge_count(Some(3)), Some(3));
    }

    /// The pixel at (x, y)
    fn at(pixels: &[u8], x: usize, y: usize) -> [u8; 4] {
        let i = (y * SIZE as usize + x) * 4;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    }

    #[test]
    fn the_overlay_icon_is_a_red_disc_with_the_count() {
        for count in [1, 7, 12, 400] {
            let pixels = overlay_icon(count);
            assert_eq!(pixels.len(), (SIZE * SIZE * 4) as usize);
            // Clear corners, red edge, white text in the middle
            assert_eq!(at(&pixels, 0, 0)[3], 0, "{count}");
            assert_eq!(at(&pixels, 8, 0), RED, "{count}");
            let white = (0..SIZE as usize * SIZE as usize)
                .filter(|i| at(&pixels, i % 16, i / 16) == WHITE)
                .count();
            assert!(white > 5, "{count}: the count is drawn");
        }
        // Past nine, every count looks the same ("9+")
        assert_eq!(overlay_icon(12), overlay_icon(400));
        assert_ne!(overlay_icon(1), overlay_icon(7));
    }
}

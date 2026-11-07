//! Flyout placement math, kept pure so it is testable without a window.

/// Physical-pixel rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Top-left for a `w`×`h` flyout: centered over the tray icon, against the taskbar edge
/// nearest the icon (bottom or top), never outside the work area.
pub fn place(icon: Area, w: i32, h: i32, work: Area, gap: i32) -> (i32, i32) {
    let x = (icon.x + icon.w / 2 - w / 2)
        .min(work.x + work.w - w - gap)
        .max(work.x + gap);
    let bottom = icon.y + icon.h / 2 >= work.y + work.h / 2;
    let y = if bottom { work.y + work.h - h - gap } else { work.y + gap };
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: Area = Area { x: 0, y: 0, w: 1920, h: 1032 };

    #[test]
    fn place_above_bottom_taskbar_and_clamped() {
        let icon = Area { x: 1000, y: 1040, w: 24, h: 32 };
        assert_eq!(place(icon, 340, 400, WORK, 12), (842, 620));
        let edge = Area { x: 1900, ..icon };
        assert_eq!(place(edge, 340, 400, WORK, 12), (1568, 620));
        let top = Area { y: -30, ..icon };
        assert_eq!(place(top, 340, 400, Area { y: 40, ..WORK }, 12).1, 52);
    }
}

//! Badge pixels. GDI draws white text on black; that coverage becomes alpha, so the text blends cleanly over
//! the taskbar's translucent material. Output: a premultiplied BGRA bitmap in a DC, ready for UpdateLayeredWindow.
use super::Lines;
use crate::icon::cover;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::SIZE;
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, CreateFontW, DeleteDC, DeleteObject, GdiFlush, GetTextExtentPoint32W,
    SelectObject, SetBkMode, SetTextColor, TextOutW, ANTIALIASED_QUALITY, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DIB_RGB_COLORS, FW_NORMAL, FW_SEMIBOLD, HBITMAP, HDC, HFONT, HGDIOBJ,
    OUT_DEFAULT_PRECIS, TRANSPARENT,
};

const ORANGE: [u8; 3] = [0xFF, 0x8A, 0x1F];
const GREY: [u8; 3] = [0x8B, 0x8B, 0x8B];

/// How to draw: DPI scale, taskbar height, taskbar theme, pointer over the badge.
pub struct Style {
    pub scale: f32,
    pub height: i32,
    pub dark: bool,
    pub hover: bool,
}

/// A memory DC holding the finished bitmap; frees its GDI objects on drop.
pub struct Frame {
    pub dc: HDC,
    pub size: SIZE,
    bitmap: HBITMAP,
    old: HGDIOBJ,
}

impl Drop for Frame {
    fn drop(&mut self) {
        // SAFETY: these handles were created in `frame` and are released exactly once, here.
        unsafe {
            SelectObject(self.dc, self.old);
            DeleteObject(self.bitmap);
            DeleteDC(self.dc);
        }
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// Segoe UI at `px` pixels tall, grayscale antialiased (coverage must not carry ClearType color fringes).
fn font(px: i32, weight: i32) -> HFONT {
    let face: Vec<u16> = wide("Segoe UI").into_iter().chain([0]).collect();
    // SAFETY: `face` is NUL-terminated and outlives the call.
    unsafe {
        CreateFontW(-px, 0, 0, 0, weight, 0, 0, 0, DEFAULT_CHARSET as u32, OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32, ANTIALIASED_QUALITY as u32, 0, face.as_ptr())
    }
}

/// Draws `text` with `f` at (x, y) when `at` is given; returns its size either way.
fn text(dc: HDC, f: HFONT, s: &str, at: Option<(i32, i32)>) -> SIZE {
    let w = wide(s);
    let mut size = SIZE { cx: 0, cy: 0 };
    // SAFETY: `dc` and `f` are live; `w` outlives the calls and its length is passed explicitly.
    unsafe {
        let prev = SelectObject(dc, f);
        GetTextExtentPoint32W(dc, w.as_ptr(), w.len() as i32, &mut size);
        if let Some((x, y)) = at {
            TextOutW(dc, x, y, w.as_ptr(), w.len() as i32);
        }
        SelectObject(dc, prev);
    }
    size
}

/// Paints `src` over the premultiplied pixel `px` with opacity `a` (0..1).
fn blend(px: &mut [u8], src: [u8; 3], a: f32) {
    let [r, g, b] = src.map(f32::from);
    for (c, s) in px.iter_mut().zip([b, g, r, 255.0]) {
        *c = (s * a + f32::from(*c) * (1.0 - a)).round() as u8;
    }
}

/// Renders the badge: accent dot, value line, caption line.
pub fn frame(l: &Lines, st: &Style) -> Option<Frame> {
    let s = |v: f32| (v * st.scale).round() as i32;
    let (pad, dot, gap) = (s(8.0), s(10.0), s(8.0));
    // SAFETY: plain GDI object creation; every handle is checked and owned by the returned Frame or freed here.
    unsafe {
        let dc = CreateCompatibleDC(null_mut());
        if dc.is_null() {
            return None;
        }
        let (big, small) = (font(s(14.0), FW_SEMIBOLD as i32), font(s(12.0), FW_NORMAL as i32));
        let (v, c) = (text(dc, big, &l.value, None), text(dc, small, &l.caption, None));
        let size = SIZE { cx: pad + dot + gap + v.cx.max(c.cx) + pad, cy: st.height };
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size.cx,
            biHeight: -size.cy,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..std::mem::zeroed()
        };
        let mut bits = null_mut();
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if bitmap.is_null() || bits.is_null() {
            DeleteObject(big);
            DeleteObject(small);
            DeleteDC(dc);
            return None;
        }
        let old = SelectObject(dc, bitmap);
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, 0x00FF_FFFF);
        let (x, top) = (pad + dot + gap, (size.cy - v.cy - c.cy) / 2);
        text(dc, big, &l.value, Some((x, top)));
        text(dc, small, &l.caption, Some((x, top + v.cy)));
        DeleteObject(big);
        DeleteObject(small);
        GdiFlush();
        let px = std::slice::from_raw_parts_mut(bits.cast::<u8>(), (size.cx * size.cy * 4) as usize);
        compose(px, size, st, top + v.cy, (pad, dot), l.active);
        Some(Frame { dc, size, bitmap, old })
    }
}

/// Turns GDI's white-on-black text into premultiplied ink over a near-clear (still clickable) background,
/// then adds the hover plate and the accent dot.
fn compose(px: &mut [u8], size: SIZE, st: &Style, caption_top: i32, (pad, dot): (i32, i32), active: bool) {
    let ink = if st.dark { [0xFF; 3] } else { [0x1B; 3] };
    let caption = if st.dark { 0.72 } else { 0.62 };
    let (w, h) = (size.cx as f32, size.cy as f32);
    let inset = (4.0 * st.scale).round();
    let plate = (0.0, inset, w, h - inset);
    let (dx, dy) = (pad as f32, (h - dot as f32) / 2.0);
    let dot_box = (dx, dy, dx + dot as f32, dy + dot as f32);
    for (i, p) in px.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let (x, y) = ((i as i32 % size.cx) as f32 + 0.5, (i as i32 / size.cx) as f32 + 0.5);
        let coverage = f32::from(p[0].max(p[1]).max(p[2])) / 255.0;
        // Alpha 1/255 everywhere: invisible, but clicks between letters still land on the badge.
        p.copy_from_slice(&[0, 0, 0, 1]);
        if st.hover {
            blend(p, ink, 0.08 * cover((x, y), plate, 4.0 * st.scale));
        }
        blend(p, ink, coverage * if y < caption_top as f32 { 1.0 } else { caption });
        blend(p, if active { ORANGE } else { GREY }, cover((x, y), dot_box, dot as f32 / 2.0));
    }
}

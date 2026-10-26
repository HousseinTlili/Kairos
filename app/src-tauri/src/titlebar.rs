//! The window's own title bar in the charter's colours (docs/design/BRAND.md): charcoal caption,
//! vellum text, a dark-gold edge. Windows's real title bar is kept (snap layouts, the maximise
//! menu, dragging) and only coloured, through DWM's Windows 11 attributes; older Windows ignores
//! them and keeps its default bar.

/// A Windows COLORREF (0x00BBGGRR) from "#RRGGBB" parts.
pub const fn colorref(r: u8, g: u8, b: u8) -> u32 {
    (b as u32) << 16 | (g as u32) << 8 | r as u32
}

/// Charcoal, as the sidebar (`--sidebar-bg` in global.css).
pub const CAPTION: u32 = colorref(0x15, 0x14, 0x13);
/// Vellum.
pub const TEXT: u32 = colorref(0xec, 0xe4, 0xd3);
/// Gold leaf, darkened so the edge is a line, not a frame.
pub const BORDER: u32 = colorref(0x5c, 0x4a, 0x24);

#[cfg(windows)]
pub fn paint<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) {
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DwmSetWindowAttribute,
    };
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    for (attribute, color) in [
        (DWMWA_CAPTION_COLOR, CAPTION),
        (DWMWA_TEXT_COLOR, TEXT),
        (DWMWA_BORDER_COLOR, BORDER),
    ] {
        // SAFETY: a live window handle, and a 4-byte COLORREF as DWM expects for these
        // attributes. A failure (Windows 10) only leaves the default colour.
        unsafe {
            DwmSetWindowAttribute(
                hwnd.0,
                attribute as u32,
                (&raw const color).cast(),
                std::mem::size_of::<u32>() as u32,
            );
        }
    }
}

#[cfg(not(windows))]
pub fn paint<R: tauri::Runtime>(_window: &tauri::WebviewWindow<R>) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_are_in_windows_byte_order() {
        assert_eq!(colorref(0x12, 0x34, 0x56), 0x0056_3412);
        assert_eq!(TEXT, 0x00d3_e4ec);
    }
}

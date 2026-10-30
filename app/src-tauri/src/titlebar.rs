//! The window's own title bar in the charter's colours (docs/design/BRAND.md): charcoal caption,
//! vellum text, a dark-gold edge. Windows's real title bar is kept (snap layouts, the maximise
//! menu, dragging) and only coloured, through DWM's Windows 11 attributes; older Windows ignores
//! them and keeps its default bar.

/// A Windows COLORREF (0x00BBGGRR) from "#RRGGBB" parts.
pub const fn colorref(r: u8, g: u8, b: u8) -> u32 {
    (b as u32) << 16 | (g as u32) << 8 | r as u32
}

/// Charcoal, as the sidebar (`--sidebar-bg` in global.css).
pub const CAPTION: u32 = colorref(0x0c, 0x0b, 0x0a);
/// Vellum.
pub const TEXT: u32 = colorref(0xec, 0xe4, 0xd3);
/// Gold leaf, darkened so the edge is a line, not a frame.
pub const BORDER: u32 = colorref(0x5c, 0x4a, 0x24);

/// The caption, its text and the edge, as COLORREFs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Colors {
    pub caption: u32,
    pub text: u32,
    pub border: u32,
}

/// The charter's own (M10), until the theme says otherwise (M16).
pub const CHARTER: Colors = Colors {
    caption: CAPTION,
    text: TEXT,
    border: BORDER,
};

/// "#rrggbb" to a COLORREF; anything else is refused.
pub fn parse_hex(text: &str) -> Option<u32> {
    let hex = text.trim().strip_prefix('#')?;
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let n = u32::from_str_radix(hex, 16).ok()?;
    Some(colorref((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

#[cfg(windows)]
pub fn paint<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) {
    paint_with(window, CHARTER);
}

#[cfg(windows)]
pub fn paint_with<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>, colors: Colors) {
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DwmSetWindowAttribute,
    };
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    for (attribute, color) in [
        (DWMWA_CAPTION_COLOR, colors.caption),
        (DWMWA_TEXT_COLOR, colors.text),
        (DWMWA_BORDER_COLOR, colors.border),
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

#[cfg(not(windows))]
pub fn paint_with<R: tauri::Runtime>(_window: &tauri::WebviewWindow<R>, _colors: Colors) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_are_in_windows_byte_order() {
        assert_eq!(colorref(0x12, 0x34, 0x56), 0x0056_3412);
        assert_eq!(TEXT, 0x00d3_e4ec);
    }

    #[test]
    fn a_theme_colour_is_read_from_its_hex() {
        assert_eq!(parse_hex("#ece4d3"), Some(TEXT));
        assert_eq!(parse_hex(" #0C0B0A "), Some(CAPTION));
        assert_eq!(parse_hex("ece4d3"), None);
        assert_eq!(parse_hex("#ece4d"), None);
        assert_eq!(parse_hex("#gggggg"), None);
    }
}

//! Icons through the Windows shell, as PNG data URLs. The caller must be on a thread that has
//! initialised COM. Nothing here knows which feature asked.
use super::png;
use std::ffi::c_void;
use windows::{
    Win32::{
        Foundation::SIZE,
        Graphics::Gdi::{
            BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC,
            GetDIBits, GetObjectW, HGDIOBJ, ReleaseDC,
        },
        UI::Shell::{
            IShellItem, IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_ICONONLY,
        },
    },
    core::{Interface, PCWSTR},
};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The icon of whatever `parsing_name` names (a file path, or `shell:AppsFolder\<AppUserModelID>`
/// for a Store app), `px` x `px`. `None` if the shell cannot produce one.
pub fn icon_data_url(parsing_name: &str, px: i32) -> Option<String> {
    let name = wide(parsing_name);
    // SAFETY: shell/GDI calls on valid handles; the HBITMAP is deleted on every path below.
    unsafe {
        let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(name.as_ptr()), None).ok()?;
        let factory: IShellItemImageFactory = item.cast().ok()?;
        let hbmp = factory
            .GetImage(SIZE { cx: px, cy: px }, SIIGBF_ICONONLY)
            .ok()?;
        let result = (|| {
            let mut bm = BITMAP::default();
            if GetObjectW(
                HGDIOBJ(hbmp.0),
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut bm as *mut _ as *mut c_void),
            ) == 0
            {
                return None;
            }
            let (w, h) = (bm.bmWidth, bm.bmHeight);
            if w <= 0 || h <= 0 {
                return None;
            }
            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h, // top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bgra = vec![0u8; w as usize * h as usize * 4];
            let dc = GetDC(None);
            let got = GetDIBits(
                dc,
                hbmp,
                0,
                h as u32,
                Some(bgra.as_mut_ptr() as *mut c_void),
                &mut bmi,
                DIB_RGB_COLORS,
            );
            ReleaseDC(None, dc);
            if got == 0 {
                return None;
            }
            let rgba = png::bgra_premultiplied_to_rgba(&bgra);
            png::png_data_url(w as u32, h as u32, &rgba).ok()
        })();
        let _ = DeleteObject(HGDIOBJ(hbmp.0));
        result
    }
}

//! Cover art: recognise the image type from its first bytes and wrap it as a `data:` URL for the
//! webview. No image crate (CLAUDE.md rule 10): we never decode, only identify and pass through.

/// Largest cover we will pass on. A cover is a small image; anything bigger is skipped rather than
/// pushed through IPC and the webview.
pub const MAX_BYTES: usize = 3 * 1024 * 1024;

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16
            | (*chunk.get(1).unwrap_or(&0) as u32) << 8
            | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            B64[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// The MIME type, judged from the magic bytes only (never from a file name or what the app claims).
pub fn sniff(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, ..] => Some("image/png"),
        [0xFF, 0xD8, 0xFF, ..] => Some("image/jpeg"),
        [b'G', b'I', b'F', b'8', b'7' | b'9', b'a', ..] => Some("image/gif"),
        [b'B', b'M', ..] if bytes.len() >= 26 => Some("image/bmp"),
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => Some("image/webp"),
        _ => None,
    }
}

/// `data:<mime>;base64,...`, or `None` if the bytes are empty, too large, or not a known image.
pub fn data_url(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return None;
    }
    let mime = sniff(bytes)?;
    Some(format!("data:{mime};base64,{}", base64(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

    #[test]
    fn recognises_each_supported_type_by_its_magic_bytes() {
        assert_eq!(sniff(&PNG), Some("image/png"));
        assert_eq!(
            sniff(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10]),
            Some("image/jpeg")
        );
        assert_eq!(sniff(b"GIF89a....."), Some("image/gif"));
        assert_eq!(sniff(b"GIF87a....."), Some("image/gif"));
        let mut bmp = vec![b'B', b'M'];
        bmp.resize(30, 0);
        assert_eq!(sniff(&bmp), Some("image/bmp"));
        assert_eq!(sniff(b"RIFF\x24\0\0\0WEBPVP8 "), Some("image/webp"));
    }

    #[test]
    fn does_not_guess() {
        assert_eq!(sniff(b""), None);
        assert_eq!(sniff(b"\x89PN"), None); // truncated PNG signature
        assert_eq!(sniff(b"<svg xmlns='http://www.w3.org/2000/svg'/>"), None); // active content
        assert_eq!(sniff(b"<html>"), None);
        assert_eq!(sniff(b"RIFF\0\0\0\0WAVEfmt "), None); // RIFF but not WebP
        assert_eq!(sniff(b"BM"), None); // too short to be a bitmap
        assert_eq!(sniff(b"GIF90a"), None);
    }

    #[test]
    fn data_url_has_the_sniffed_type_and_valid_base64() {
        let mut png = PNG.to_vec();
        png.extend_from_slice(b"rest");
        let url = data_url(&png).unwrap();
        assert!(url.starts_with("data:image/png;base64,"));
        assert_eq!(url, format!("data:image/png;base64,{}", base64(&png)));
    }

    #[test]
    fn base64_matches_the_rfc_vectors() {
        for (i, o) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(i.as_bytes()), o);
        }
    }

    #[test]
    fn empty_oversized_and_unknown_data_become_none() {
        assert_eq!(data_url(&[]), None);
        assert_eq!(data_url(b"not an image at all"), None);
        let mut big = PNG.to_vec();
        big.resize(MAX_BYTES + 1, 0);
        assert_eq!(data_url(&big), None);
        let mut ok = PNG.to_vec();
        ok.resize(MAX_BYTES, 0);
        assert!(data_url(&ok).is_some());
    }
}

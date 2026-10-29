//! Tiny encoders so thumbnails can travel as `data:` URLs without an image or base64 crate.

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64(bytes: &[u8]) -> String {
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

/// Size in bytes of one 24-bit BMP row (rows are padded to 4 bytes).
pub fn bmp24_row_bytes(width: u32) -> usize {
    ((width as usize * 3).div_ceil(4)) * 4
}

/// Wrap bottom-up, 4-byte-padded BGR rows (as `GetDIBits` returns them) in a BMP file.
pub fn bmp24(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>, String> {
    let expected = bmp24_row_bytes(width) * height as usize;
    if pixels.len() != expected {
        return Err(format!(
            "pixel buffer is {} bytes, expected {expected}",
            pixels.len()
        ));
    }
    let offset = 14 + 40;
    let mut out = Vec::with_capacity(offset + pixels.len());
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&((offset + pixels.len()) as u32).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(offset as u32).to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes()); // BITMAPINFOHEADER
    out.extend_from_slice(&(width as i32).to_le_bytes());
    out.extend_from_slice(&(height as i32).to_le_bytes()); // positive = bottom-up
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&24u16.to_le_bytes()); // bits per pixel
    out.extend_from_slice(&[0; 24]); // compression, size, resolution, colours
    out.extend_from_slice(pixels);
    Ok(out)
}

pub fn bmp_data_url(width: u32, height: u32, pixels: &[u8]) -> Result<String, String> {
    Ok(format!(
        "data:image/bmp;base64,{}",
        base64(&bmp24(width, height, pixels)?)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648_vectors() {
        for (input, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(input.as_bytes()), expected, "input {input:?}");
        }
    }

    #[test]
    fn rows_are_padded_to_four_bytes() {
        assert_eq!(bmp24_row_bytes(1), 4);
        assert_eq!(bmp24_row_bytes(2), 8);
        assert_eq!(bmp24_row_bytes(4), 12);
        assert_eq!(bmp24_row_bytes(160), 480);
    }

    #[test]
    fn bmp_header_is_well_formed() {
        let px = vec![0u8; bmp24_row_bytes(3) * 2];
        let b = bmp24(3, 2, &px).unwrap();
        assert_eq!(&b[0..2], b"BM");
        assert_eq!(
            u32::from_le_bytes(b[2..6].try_into().unwrap()) as usize,
            b.len()
        );
        assert_eq!(u32::from_le_bytes(b[10..14].try_into().unwrap()), 54);
        assert_eq!(i32::from_le_bytes(b[18..22].try_into().unwrap()), 3);
        assert_eq!(i32::from_le_bytes(b[22..26].try_into().unwrap()), 2);
        assert_eq!(u16::from_le_bytes(b[28..30].try_into().unwrap()), 24);
    }

    #[test]
    fn wrong_buffer_size_is_an_error_not_a_panic() {
        assert!(bmp24(3, 2, &[0; 5]).is_err());
    }
}

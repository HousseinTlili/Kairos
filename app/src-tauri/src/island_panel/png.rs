//! A tiny PNG writer and base64 so icons can travel as `data:` URLs without an image crate
//! (rule 10). The image data is stored in uncompressed deflate blocks; icons are small (64x64), so
//! a ~16 KB data URL each is fine and they are cached in memory.

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in bytes {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// Encode straight (non-premultiplied) RGBA, row by row, top to bottom.
pub fn rgba_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    if width == 0 || height == 0 || rgba.len() != width as usize * height as usize * 4 {
        return Err(format!(
            "pixel buffer is {} bytes, expected {}x{}x4",
            rgba.len(),
            width,
            height
        ));
    }
    // Each scanline is prefixed with filter type 0 (none).
    let mut raw = Vec::with_capacity(rgba.len() + height as usize);
    for row in rgba.chunks(width as usize * 4) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut z = vec![0x78, 0x01];
    let mut blocks = raw.chunks(65_535).peekable();
    while let Some(b) = blocks.next() {
        z.push(u8::from(blocks.peek().is_none())); // BFINAL, BTYPE = 00 (stored)
        z.extend_from_slice(&(b.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(b.len() as u16)).to_le_bytes());
        z.extend_from_slice(b);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8 bit, RGBA, deflate, no filter, no interlace
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    Ok(out)
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            B64[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            B64[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

pub fn png_data_url(width: u32, height: u32, rgba: &[u8]) -> Result<String, String> {
    Ok(format!(
        "data:image/png;base64,{}",
        base64(&rgba_png(width, height, rgba)?)
    ))
}

/// Windows hands icons back premultiplied (BGRA). Convert to straight RGBA.
pub fn bgra_premultiplied_to_rgba(bgra: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bgra.len());
    for px in bgra.as_chunks::<4>().0 {
        let a = px[3] as u32;
        let un = |c: u8| -> u8 {
            if a == 0 || a == 255 {
                c
            } else {
                ((c as u32 * 255 + a / 2) / a).min(255) as u8
            }
        };
        out.extend_from_slice(&[un(px[2]), un(px[1]), un(px[0]), px[3]]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_and_adler_match_known_values() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn base64_matches_rfc_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    /// Walk the chunks, verify every CRC, and read the pixels back out of the stored blocks.
    fn decode(png: &[u8]) -> (u32, u32, Vec<u8>) {
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        let (mut i, mut w, mut h, mut idat) = (8, 0, 0, Vec::new());
        let mut kinds = Vec::new();
        while i < png.len() {
            let len = u32::from_be_bytes(png[i..i + 4].try_into().unwrap()) as usize;
            let kind: [u8; 4] = png[i + 4..i + 8].try_into().unwrap();
            let data = &png[i + 8..i + 8 + len];
            let crc = u32::from_be_bytes(png[i + 8 + len..i + 12 + len].try_into().unwrap());
            assert_eq!(crc, crc32(&png[i + 4..i + 8 + len]), "crc of {kind:?}");
            match &kind {
                b"IHDR" => {
                    w = u32::from_be_bytes(data[0..4].try_into().unwrap());
                    h = u32::from_be_bytes(data[4..8].try_into().unwrap());
                    assert_eq!(&data[8..], &[8, 6, 0, 0, 0]);
                }
                b"IDAT" => idat.extend_from_slice(data),
                _ => {}
            }
            kinds.push(kind);
            i += 12 + len;
        }
        assert_eq!(kinds, vec![*b"IHDR", *b"IDAT", *b"IEND"]);
        // zlib header, stored blocks, adler32
        assert_eq!(&idat[..2], &[0x78, 0x01]);
        let (mut p, mut raw) = (2, Vec::new());
        loop {
            let last = idat[p] & 1 == 1;
            let len = u16::from_le_bytes([idat[p + 1], idat[p + 2]]) as usize;
            let nlen = u16::from_le_bytes([idat[p + 3], idat[p + 4]]);
            assert_eq!(nlen, !(len as u16));
            raw.extend_from_slice(&idat[p + 5..p + 5 + len]);
            p += 5 + len;
            if last {
                break;
            }
        }
        assert_eq!(
            u32::from_be_bytes(idat[p..p + 4].try_into().unwrap()),
            adler32(&raw)
        );
        let mut px = Vec::new();
        for row in raw.chunks(w as usize * 4 + 1) {
            assert_eq!(row[0], 0, "filter byte");
            px.extend_from_slice(&row[1..]);
        }
        (w, h, px)
    }

    #[test]
    fn small_image_round_trips() {
        let rgba: Vec<u8> = (0..4 * 3 * 4).map(|i| (i * 7) as u8).collect();
        let png = rgba_png(4, 3, &rgba).unwrap();
        assert_eq!(decode(&png), (4, 3, rgba));
    }

    #[test]
    fn large_image_spans_several_stored_blocks() {
        // 200x200 RGBA = 160 KB of pixels: more than two 65535-byte blocks.
        let rgba: Vec<u8> = (0..200 * 200 * 4).map(|i| (i % 251) as u8).collect();
        let png = rgba_png(200, 200, &rgba).unwrap();
        assert_eq!(decode(&png), (200, 200, rgba));
    }

    #[test]
    fn wrong_buffer_sizes_are_rejected() {
        assert!(rgba_png(2, 2, &[0; 15]).is_err());
        assert!(rgba_png(0, 2, &[]).is_err());
    }

    #[test]
    fn unpremultiply_restores_colours() {
        // 50% alpha, premultiplied BGRA (16,32,64) -> straight RGBA (128,64,32): value * 255 / alpha.
        let out = bgra_premultiplied_to_rgba(&[16, 32, 64, 128, 1, 2, 3, 255, 9, 9, 9, 0]);
        assert_eq!(&out[..4], &[128, 64, 32, 128]);
        assert_eq!(&out[4..8], &[3, 2, 1, 255]); // opaque: only channel order changes
        assert_eq!(&out[8..], &[9, 9, 9, 0]);
    }
}

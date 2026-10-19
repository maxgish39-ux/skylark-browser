//! Bounded image decoding for the browser's own paint pipeline.
//! Decoded images are capped by dimensions and pixel count before allocation.

use image::{ImageReader, Limits};
use std::io::Cursor;
use url::Url;

pub const MAX_IMAGE_DIMENSION: u32 = 2048;
pub const MAX_IMAGE_PIXELS: u64 = 4_194_304;

/// Resolve an image URL using the final URL of the containing document.
pub fn resolve_image_url(base: &str, source: &str) -> Result<String, String> {
    let source = source.trim();
    if source.is_empty() { return Err("empty image source".to_owned()); }
    let base = Url::parse(base).map_err(|e| format!("invalid document URL: {e}"))?;
    if !matches!(base.scheme(), "http" | "https") { return Err("image resources require an HTTP(S) document".to_owned()); }
    let resolved = base.join(source).map_err(|e| format!("invalid image URL: {e}"))?;
    if !matches!(resolved.scheme(), "http" | "https") || resolved.host_str().is_none() {
        return Err("image URL must use HTTP or HTTPS".to_owned());
    }
    Ok(resolved.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    /// Opaque 0x00RRGGBB pixels. Alpha is composited onto white during decoding.
    pub pixels: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageDecodeError {
    UnknownFormat,
    InvalidImage(String),
    DimensionsTooLarge { width: u32, height: u32 },
    TooManyPixels { pixels: u64 },
}

impl std::fmt::Display for ImageDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownFormat => write!(f, "unsupported or unrecognized image format"),
            Self::InvalidImage(message) => write!(f, "image decode failed: {message}"),
            Self::DimensionsTooLarge { width, height } => write!(f, "image dimensions {width}x{height} exceed the {MAX_IMAGE_DIMENSION}px limit"),
            Self::TooManyPixels { pixels } => write!(f, "image contains {pixels} pixels, above the {MAX_IMAGE_PIXELS}-pixel limit"),
        }
    }
}

impl std::error::Error for ImageDecodeError {}

pub fn decode_image(bytes: &[u8]) -> Result<DecodedImage, ImageDecodeError> {
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()
        .map_err(|e| ImageDecodeError::InvalidImage(e.to_string()))?;
    if reader.format().is_none() { return Err(ImageDecodeError::UnknownFormat); }
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some((MAX_IMAGE_PIXELS * 4) as u64);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|e| ImageDecodeError::InvalidImage(e.to_string()))?;
    let (width, height) = (decoded.width(), decoded.height());
    if width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
        return Err(ImageDecodeError::DimensionsTooLarge { width, height });
    }
    let count = u64::from(width).saturating_mul(u64::from(height));
    if count > MAX_IMAGE_PIXELS { return Err(ImageDecodeError::TooManyPixels { pixels: count }); }
    let rgba = decoded.to_rgba8();
    let pixels = rgba.pixels().map(|p| {
        let a = u32::from(p[3]);
        let blend = |c: u8| ((u32::from(c) * a + 255 * (255 - a) + 127) / 255) & 0xff;
        (blend(p[0]) << 16) | (blend(p[1]) << 8) | blend(p[2])
    }).collect();
    Ok(DecodedImage { width, height, pixels })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_relative_and_absolute_image_urls() {
        let base = "https://example.com/dir/page.html";
        assert_eq!(resolve_image_url(base, "../img/a.png").unwrap(), "https://example.com/img/a.png");
        assert_eq!(resolve_image_url(base, "/root.png").unwrap(), "https://example.com/root.png");
        assert!(resolve_image_url(base, "data:image/png;base64,AAAA").is_err());
        assert!(resolve_image_url(base, "").is_err());
    }

    #[test]
    fn rejects_garbage_without_panicking() {
        assert!(decode_image(b"not an image").is_err());
    }

    #[test]
    fn decodes_a_small_png_deterministically() {
        // A 1x1 opaque red PNG.
        let bytes = [137,80,78,71,13,10,26,10,0,0,0,13,73,72,68,82,0,0,0,1,0,0,0,1,8,6,0,0,0,31,21,196,137,0,0,0,13,73,68,65,84,8,215,99,248,207,192,240,31,0,5,0,1,255,137,153,61,29,0,0,0,0,73,69,78,68,174,66,96,130];
        let image = decode_image(&bytes).expect("valid fixture PNG");
        assert_eq!((image.width, image.height), (1, 1));
        assert_eq!(image.pixels, vec![0x00FF0000]);
    }
}

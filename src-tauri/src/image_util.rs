use image::imageops::FilterType;
use image::{GenericImageView, ImageFormat};
use pinyin::ToPinyin;
use sha2::{Digest, Sha256};
use std::io::Cursor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetectedImage {
    pub ext: &'static str,
    pub mime: &'static str,
    pub format: ImageFormat,
}

/// Detect supported image formats strictly by magic bytes (works even for extensionless QQ cache files).
pub fn detect_image_format(bytes: &[u8]) -> Option<DetectedImage> {
    if bytes.len() < 12 {
        return None;
    }

    // PNG: 89 50 4E 47 0D 0A 1A 0A
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some(DetectedImage {
            ext: "png",
            mime: "image/png",
            format: ImageFormat::Png,
        });
    }

    // JPEG: FF D8 FF
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(DetectedImage {
            ext: "jpg",
            mime: "image/jpeg",
            format: ImageFormat::Jpeg,
        });
    }

    // GIF: GIF87a or GIF89a
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some(DetectedImage {
            ext: "gif",
            mime: "image/gif",
            format: ImageFormat::Gif,
        });
    }

    // WebP: RIFF....WEBP
    if &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some(DetectedImage {
            ext: "webp",
            mime: "image/webp",
            format: ImageFormat::WebP,
        });
    }

    None
}

/// Detect whether a GIF or WebP contains animation frames.
pub fn detect_is_animated(bytes: &[u8], ext: &str) -> bool {
    match ext {
        "gif" => is_animated_gif(bytes),
        "webp" => is_animated_webp(bytes),
        _ => false,
    }
}

fn is_animated_gif(bytes: &[u8]) -> bool {
    // Count Graphic Control Extension blocks (0x21 0xF9 0x04) or multiple image descriptors
    let mut gce_count = 0usize;
    let mut i = 0;
    while i + 3 < bytes.len() {
        if bytes[i] == 0x21 && bytes[i + 1] == 0xF9 && bytes[i + 2] == 0x04 {
            gce_count += 1;
            if gce_count >= 2 {
                return true;
            }
            i += 8;
        } else {
            i += 1;
        }
    }
    false
}

fn is_animated_webp(bytes: &[u8]) -> bool {
    if bytes.len() < 30 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return false;
    }
    // Extended WebP VP8X chunk has animation flag at bit 1 (0x02) of byte 20
    if &bytes[12..16] == b"VP8X" && (bytes[20] & 0x02) != 0 {
        return true;
    }
    // Fallback: scan for ANIM chunk header
    bytes.windows(4).any(|w| w == b"ANIM")
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Inspect image dimensions and generate a lightweight static WebP thumbnail (max 160x160).
pub fn inspect_and_make_thumbnail(
    bytes: &[u8],
    format: ImageFormat,
    max_dim: u32,
) -> Result<(u32, u32, Vec<u8>), String> {
    let img = image::load_from_memory_with_format(bytes, format)
        .or_else(|_| image::load_from_memory(bytes))
        .map_err(|e| format!("解码图片失败: {e}"))?;

    let (w, h) = img.dimensions();
    let thumb = if w > max_dim || h > max_dim {
        img.resize(max_dim, max_dim, FilterType::Triangle)
    } else {
        img
    };

    let mut out = Vec::new();
    thumb
        .write_to(&mut Cursor::new(&mut out), ImageFormat::WebP)
        .map_err(|e| format!("生成 WebP 缩略图失败: {e}"))?;

    Ok((w, h, out))
}

/// Extract full pinyin and pinyin initials for Chinese characters while preserving ASCII tokens.
pub fn extract_pinyin(text: &str) -> (String, String) {
    let mut full = String::new();
    let mut initials = String::new();

    for ch in text.chars() {
        if let Some(py) = ch.to_pinyin() {
            let plain = py.plain();
            full.push_str(plain);
            if let Some(first) = plain.chars().next() {
                initials.push(first);
            }
        } else if ch.is_ascii_alphanumeric() {
            let lower = ch.to_ascii_lowercase();
            full.push(lower);
            initials.push(lower);
        } else if ch.is_whitespace() || ch == '-' || ch == '_' {
            if !full.ends_with(' ') && !full.is_empty() {
                full.push(' ');
            }
            if !initials.ends_with(' ') && !initials.is_empty() {
                initials.push(' ');
            }
        }
    }

    (full.trim().to_string(), initials.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_magic_bytes_detection() {
        let png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0];
        assert_eq!(detect_image_format(&png).unwrap().ext, "png");

        let jpg = [0xFF, 0xD8, 0xFF, 0xE0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(detect_image_format(&jpg).unwrap().ext, "jpg");

        let gif = b"GIF89a\x01\x00\x01\x00\x00\x00";
        assert_eq!(detect_image_format(gif).unwrap().ext, "gif");

        let webp = b"RIFF\x20\x00\x00\x00WEBP";
        assert_eq!(detect_image_format(webp).unwrap().ext, "webp");

        assert!(detect_image_format(b"hello world!").is_none());
    }

    #[test]
    fn test_pinyin_extraction() {
        let (full, initials) = extract_pinyin("熊猫头 哈哈");
        assert_eq!(full, "xiongmaotou haha");
        assert_eq!(initials, "xmt hh");

        let (full2, initials2) = extract_pinyin("QQ表情123");
        assert_eq!(full2, "qqbiaoqing123");
        assert_eq!(initials2, "qqbq123");
    }

    #[test]
    fn test_thumbnail_generation() {
        let img = image::RgbaImage::from_pixel(320, 240, image::Rgba([255, 0, 0, 255]));
        let mut png_bytes = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png)
            .unwrap();

        let detected = detect_image_format(&png_bytes).unwrap();
        let (w, h, thumb) = inspect_and_make_thumbnail(&png_bytes, detected.format, 160).unwrap();
        assert_eq!((w, h), (320, 240));
        assert_eq!(detect_image_format(&thumb).unwrap().ext, "webp");
    }
}

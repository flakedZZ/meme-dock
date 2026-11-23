use crate::db::Database;
use crate::image_util::{detect_image_format, detect_is_animated, sha256_hex};
use crate::models::{QqDirInfo, ScanCandidate};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Discover known QQ NT / legacy QQ meme directories on macOS and Windows.
pub fn discover_qq_dirs() -> Vec<QqDirInfo> {
    let mut results = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let mut push_if_exists = |path: PathBuf, label: String| {
        if path.is_dir() {
            let key = path.to_string_lossy().to_string();
            if seen.insert(key.clone()) {
                let count = WalkDir::new(&path)
                    .max_depth(3)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file())
                    .take(500)
                    .count();
                results.push(QqDirInfo {
                    path: key,
                    label,
                    file_count_hint: count,
                });
            }
        }
    };

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            let qq_root = home
                .join("Library")
                .join("Containers")
                .join("com.tencent.qq")
                .join("Data")
                .join("Library")
                .join("Application Support")
                .join("QQ");

            if let Ok(entries) = fs::read_dir(&qq_root) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        let account = p
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let emoji_base = p.join("nt_qq").join("nt_data").join("Emoji");
                        push_if_exists(
                            emoji_base.join("emoji-fav"),
                            format!("QQ 收藏表情 ({account})"),
                        );
                        push_if_exists(
                            emoji_base.join("emoji-recv"),
                            format!("QQ 接收表情 ({account})"),
                        );
                        push_if_exists(
                            emoji_base.join("marketface"),
                            format!("QQ 商城表情 ({account})"),
                        );
                    }
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(home) = dirs::home_dir() {
            let tencent_files = home.join("Documents").join("Tencent Files");
            if let Ok(entries) = fs::read_dir(&tencent_files) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        let account = p
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let emoji_base = p.join("nt_qq").join("nt_data").join("Emoji");
                        push_if_exists(
                            emoji_base.join("emoji-fav"),
                            format!("QQ 收藏表情 ({account})"),
                        );
                        push_if_exists(
                            emoji_base.join("emoji-recv"),
                            format!("QQ 接收表情 ({account})"),
                        );
                        // Legacy QQ custom face path
                        push_if_exists(
                            p.join("Image").join("Group2"),
                            format!("QQ 群聊图片 ({account})"),
                        );
                    }
                }
            }
        }
    }

    results
}

/// Scan a directory recursively, identifying valid images strictly via magic bytes,
/// deduplicating within the scan, and flagging memes already imported in SQLite.
pub fn scan_directory_for_memes(
    dir: &Path,
    db: &Database,
    max_items: usize,
) -> Result<Vec<ScanCandidate>, String> {
    if !dir.is_dir() {
        return Err(format!("目录不存在: {}", dir.display()));
    }

    let mut file_entries: Vec<(PathBuf, u64, std::time::SystemTime)> = Vec::new();
    for entry in WalkDir::new(dir).max_depth(6).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            let len = meta.len();
            // Skip tiny (< 64B) or huge (> 25MB) files
            if (64..=25 * 1024 * 1024).contains(&len) {
                let mtime = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
                file_entries.push((entry.into_path(), len, mtime));
            }
        }
    }

    // Newest first
    file_entries.sort_by(|a, b| b.2.cmp(&a.2));

    let mut candidates = Vec::new();
    let mut seen_hashes = std::collections::HashSet::new();

    for (path, file_size, _) in file_entries {
        if candidates.len() >= max_items {
            break;
        }
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let Some(detected) = detect_image_format(&bytes) else {
            continue;
        };

        let sha = sha256_hex(&bytes);
        if !seen_hashes.insert(sha.clone()) {
            continue;
        }

        let (width, height) = image::ImageReader::with_format(
            std::io::Cursor::new(&bytes),
            detected.format,
        )
        .into_dimensions()
        .unwrap_or((0, 0));

        // Skip tiny UI icons (< 24x24) if dimensions are known
        if width > 0 && height > 0 && (width < 24 || height < 24) {
            continue;
        }

        let is_animated = detect_is_animated(&bytes, detected.ext);
        let already_imported = db.has_sha256(&sha).unwrap_or(false);
        let suggested_alias = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .filter(|s| s.len() <= 24 && !s.chars().all(|c| c.is_ascii_hexdigit()))
            .unwrap_or_default();

        candidates.push(ScanCandidate {
            path: path.to_string_lossy().to_string(),
            sha256: sha,
            ext: detected.ext.to_string(),
            mime: detected.mime.to_string(),
            width,
            height,
            file_size,
            is_animated,
            already_imported,
            suggested_alias,
        });
    }

    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageFormat;
    use std::io::Cursor;
    use tempfile::tempdir;

    #[test]
    fn test_scan_extensionless_qq_cache_files() {
        let dir = tempdir().unwrap();
        let db = Database::open(&dir.path().join("test.db")).unwrap();

        // Create an extensionless PNG file (like QQ NT cache) and a non-image file
        let img = image::RgbaImage::from_pixel(64, 64, image::Rgba([20, 120, 240, 255]));
        let mut png_bytes = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png)
            .unwrap();

        let cache_dir = dir.path().join("qq_cache");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(cache_dir.join("9f8e7d6c5b4a3210"), &png_bytes).unwrap();
        fs::write(cache_dir.join("not_an_image.dat"), vec![0u8; 256]).unwrap();

        let candidates = scan_directory_for_memes(&cache_dir, &db, 50).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].ext, "png");
        assert_eq!(candidates[0].width, 64);
        assert_eq!(candidates[0].height, 64);
        assert!(!candidates[0].already_imported);
    }
}

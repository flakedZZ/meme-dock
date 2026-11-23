use crate::db::{Database, NewMemeRecord};
use crate::image_util::{
    detect_image_format, detect_is_animated, inspect_and_make_thumbnail, sha256_hex,
};
use crate::models::{ImportResult, MemeItem};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct MemeStore {
    pub data_dir: PathBuf,
    pub db: Database,
}

impl MemeStore {
    pub fn init(data_dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(data_dir.join("originals"))
            .map_err(|e| format!("创建原图目录失败: {e}"))?;
        fs::create_dir_all(data_dir.join("thumbs"))
            .map_err(|e| format!("创建缩略图目录失败: {e}"))?;

        let db = Database::open(&data_dir.join("memedock.db"))?;
        Ok(Self { data_dir, db })
    }

    pub fn import_bytes(
        &self,
        bytes: &[u8],
        alias: &str,
        tags: &[String],
    ) -> Result<ImportOutcome, String> {
        let Some(detected) = detect_image_format(bytes) else {
            return Ok(ImportOutcome::Skipped);
        };

        let sha = sha256_hex(bytes);
        if self.db.has_sha256(&sha)? {
            return Ok(ImportOutcome::Duplicate);
        }

        let (width, height, thumb_bytes) =
            inspect_and_make_thumbnail(bytes, detected.format, 160)?;
        let is_animated = detect_is_animated(bytes, detected.ext);

        let shard = &sha[0..2];
        let rel_orig = format!("originals/{shard}/{sha}.{}", detected.ext);
        let rel_thumb = format!("thumbs/{shard}/{sha}.webp");

        let abs_orig = self.data_dir.join(&rel_orig);
        let abs_thumb = self.data_dir.join(&rel_thumb);

        if let Some(parent) = abs_orig.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        if let Some(parent) = abs_thumb.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        fs::write(&abs_orig, bytes).map_err(|e| format!("保存原图失败: {e}"))?;
        fs::write(&abs_thumb, &thumb_bytes).map_err(|e| format!("保存缩略图失败: {e}"))?;

        let item = self.db.insert_meme(
            NewMemeRecord {
                sha256: &sha,
                alias: alias.trim(),
                ext: detected.ext,
                mime: detected.mime,
                width,
                height,
                file_size: bytes.len() as u64,
                is_animated,
                original_path: &rel_orig,
                thumb_path: &rel_thumb,
                tags,
            },
            &self.data_dir,
        )?;

        Ok(ImportOutcome::Imported(item))
    }

    pub fn import_paths(&self, paths: &[PathBuf], tags: &[String]) -> ImportResult {
        let mut result = ImportResult {
            imported: 0,
            duplicates: 0,
            skipped: 0,
            errors: Vec::new(),
            added_items: Vec::new(),
        };

        let mut expanded: Vec<PathBuf> = Vec::new();
        for p in paths {
            if p.is_dir() {
                for entry in WalkDir::new(p).max_depth(6).into_iter().filter_map(|e| e.ok()) {
                    if entry.file_type().is_file() {
                        expanded.push(entry.into_path());
                    }
                }
            } else if p.is_file() {
                expanded.push(p.clone());
            }
        }

        for path in expanded {
            let bytes = match fs::read(&path) {
                Ok(b) => b,
                Err(e) => {
                    result
                        .errors
                        .push(format!("{}: {}", path.display(), e));
                    continue;
                }
            };

            let suggested_alias = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .filter(|s| s.len() <= 24 && !s.chars().all(|c| c.is_ascii_hexdigit()))
                .unwrap_or_default();

            match self.import_bytes(&bytes, &suggested_alias, tags) {
                Ok(ImportOutcome::Imported(item)) => {
                    result.imported += 1;
                    result.added_items.push(item);
                }
                Ok(ImportOutcome::Duplicate) => result.duplicates += 1,
                Ok(ImportOutcome::Skipped) => result.skipped += 1,
                Err(e) => result.errors.push(format!("{}: {}", path.display(), e)),
            }
        }

        result
    }

    pub fn delete_meme_with_files(&self, id: i64) -> Result<bool, String> {
        let Some((rel_orig, rel_thumb)) = self.db.delete_meme(id)? else {
            return Ok(false);
        };
        let _ = fs::remove_file(self.data_dir.join(rel_orig));
        let _ = fs::remove_file(self.data_dir.join(rel_thumb));
        Ok(true)
    }

    pub fn resolve_original_path(&self, id: i64) -> Result<PathBuf, String> {
        let item = self
            .db
            .get_meme_by_id(id, &self.data_dir)?
            .ok_or_else(|| "表情包不存在".to_string())?;
        let p = PathBuf::from(&item.original_path);
        if !p.exists() {
            return Err("原图文件已丢失".to_string());
        }
        Ok(p)
    }

    pub fn is_valid_dir(path: &Path) -> bool {
        path.exists() && path.is_dir()
    }
}

pub enum ImportOutcome {
    Imported(MemeItem),
    Duplicate,
    Skipped,
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageFormat;
    use std::io::Cursor;
    use tempfile::tempdir;

    #[test]
    fn test_store_dedup_and_delete() {
        let dir = tempdir().unwrap();
        let store = MemeStore::init(dir.path().to_path_buf()).unwrap();

        let img = image::RgbaImage::from_pixel(100, 100, image::Rgba([0, 180, 255, 255]));
        let mut png_bytes = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png)
            .unwrap();

        let item = match store
            .import_bytes(&png_bytes, "蓝块", &["测试".to_string()])
            .unwrap()
        {
            ImportOutcome::Imported(m) => m,
            _ => panic!("expected imported"),
        };

        assert!(Path::new(&item.original_path).exists());
        assert!(Path::new(&item.thumb_path).exists());

        // Second import of same bytes must deduplicate
        let dup = store.import_bytes(&png_bytes, "蓝块2", &[]).unwrap();
        assert!(matches!(dup, ImportOutcome::Duplicate));

        // Delete removes DB record and files
        assert!(store.delete_meme_with_files(item.id).unwrap());
        assert!(!Path::new(&item.original_path).exists());
        assert!(!Path::new(&item.thumb_path).exists());
    }
}

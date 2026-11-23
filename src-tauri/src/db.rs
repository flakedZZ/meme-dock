use crate::image_util::extract_pinyin;
use crate::models::{AppSettings, MemeItem, MemePage, TagInfo};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Database {
    conn: Mutex<Connection>,
}

pub struct NewMemeRecord<'a> {
    pub sha256: &'a str,
    pub alias: &'a str,
    pub ext: &'a str,
    pub mime: &'a str,
    pub width: u32,
    pub height: u32,
    pub file_size: u64,
    pub is_animated: bool,
    pub original_path: &'a str,
    pub thumb_path: &'a str,
    pub tags: &'a [String],
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Database {
    pub fn open(db_path: &Path) -> Result<Self, String> {
        let conn = Connection::open(db_path).map_err(|e| format!("打开数据库失败: {e}"))?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS memes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                sha256 TEXT NOT NULL UNIQUE,
                alias TEXT NOT NULL DEFAULT '',
                pinyin_full TEXT NOT NULL DEFAULT '',
                pinyin_initials TEXT NOT NULL DEFAULT '',
                ext TEXT NOT NULL,
                mime TEXT NOT NULL,
                width INTEGER NOT NULL DEFAULT 0,
                height INTEGER NOT NULL DEFAULT 0,
                file_size INTEGER NOT NULL DEFAULT 0,
                is_animated INTEGER NOT NULL DEFAULT 0,
                original_path TEXT NOT NULL,
                thumb_path TEXT NOT NULL,
                use_count INTEGER NOT NULL DEFAULT 0,
                last_used_at INTEGER,
                created_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_memes_created_at ON memes(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_memes_use_count ON memes(use_count DESC, last_used_at DESC);

            CREATE TABLE IF NOT EXISTS tags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                pinyin_full TEXT NOT NULL DEFAULT '',
                pinyin_initials TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS meme_tags (
                meme_id INTEGER NOT NULL REFERENCES memes(id) ON DELETE CASCADE,
                tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
                PRIMARY KEY (meme_id, tag_id)
            );

            CREATE INDEX IF NOT EXISTS idx_meme_tags_tag_id ON meme_tags(tag_id);

            CREATE VIRTUAL TABLE IF NOT EXISTS memes_fts USING fts5(
                alias,
                pinyin_full,
                pinyin_initials,
                tags_text,
                tokenize='unicode61'
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )
        .map_err(|e| format!("初始化数据库表结构失败: {e}"))?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn has_sha256(&self, sha256: &str) -> Result<bool, String> {
        let conn = self.conn.lock();
        let exists: Option<i64> = conn
            .query_row(
                "SELECT id FROM memes WHERE sha256 = ?1",
                params![sha256],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        Ok(exists.is_some())
    }

    pub fn insert_meme(&self, rec: NewMemeRecord<'_>, data_dir: &Path) -> Result<MemeItem, String> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let (py_full, py_init) = extract_pinyin(rec.alias);
        let created_at = now_ts();

        tx.execute(
            "INSERT INTO memes (
                sha256, alias, pinyin_full, pinyin_initials, ext, mime,
                width, height, file_size, is_animated, original_path, thumb_path, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                rec.sha256,
                rec.alias,
                py_full,
                py_init,
                rec.ext,
                rec.mime,
                rec.width,
                rec.height,
                rec.file_size as i64,
                if rec.is_animated { 1 } else { 0 },
                rec.original_path,
                rec.thumb_path,
                created_at
            ],
        )
        .map_err(|e| format!("写入表情记录失败: {e}"))?;

        let meme_id = tx.last_insert_rowid();
        set_meme_tags_tx(&tx, meme_id, rec.tags)?;
        sync_fts_tx(&tx, meme_id)?;
        tx.commit().map_err(|e| e.to_string())?;

        drop(conn);
        self.get_meme_by_id(meme_id, data_dir)?
            .ok_or_else(|| "插入后读取表情失败".to_string())
    }

    pub fn get_meme_by_id(&self, id: i64, data_dir: &Path) -> Result<Option<MemeItem>, String> {
        let conn = self.conn.lock();
        get_meme_by_id_conn(&conn, id, data_dir)
    }

    pub fn update_meme_meta(
        &self,
        id: i64,
        alias: &str,
        tags: &[String],
        data_dir: &Path,
    ) -> Result<MemeItem, String> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let trimmed = alias.trim();
        let (py_full, py_init) = extract_pinyin(trimmed);

        let changed = tx
            .execute(
                "UPDATE memes SET alias = ?1, pinyin_full = ?2, pinyin_initials = ?3 WHERE id = ?4",
                params![trimmed, py_full, py_init, id],
            )
            .map_err(|e| e.to_string())?;

        if changed == 0 {
            return Err("未找到对应的表情包".to_string());
        }

        set_meme_tags_tx(&tx, id, tags)?;
        sync_fts_tx(&tx, id)?;
        tx.commit().map_err(|e| e.to_string())?;

        drop(conn);
        self.get_meme_by_id(id, data_dir)?
            .ok_or_else(|| "更新后读取表情失败".to_string())
    }

    pub fn delete_meme(&self, id: i64) -> Result<Option<(String, String)>, String> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        let paths: Option<(String, String)> = tx
            .query_row(
                "SELECT original_path, thumb_path FROM memes WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;

        if paths.is_none() {
            return Ok(None);
        }

        tx.execute("DELETE FROM memes_fts WHERE rowid = ?1", params![id])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM memes WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        // Clean up orphaned tags
        tx.execute(
            "DELETE FROM tags WHERE id NOT IN (SELECT DISTINCT tag_id FROM meme_tags)",
            [],
        )
        .map_err(|e| e.to_string())?;

        tx.commit().map_err(|e| e.to_string())?;
        Ok(paths)
    }

    pub fn record_use(&self, id: i64) -> Result<(), String> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE memes SET use_count = use_count + 1, last_used_at = ?1 WHERE id = ?2",
            params![now_ts(), id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn list_tags(&self) -> Result<Vec<TagInfo>, String> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT t.id, t.name, COUNT(mt.meme_id) AS cnt
                 FROM tags t
                 INNER JOIN meme_tags mt ON mt.tag_id = t.id
                 GROUP BY t.id, t.name
                 ORDER BY cnt DESC, t.name ASC",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                Ok(TagInfo {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    count: row.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;

        let mut tags = Vec::new();
        for r in rows {
            tags.push(r.map_err(|e| e.to_string())?);
        }
        Ok(tags)
    }

    pub fn query_memes(
        &self,
        query: &str,
        tags: &[String],
        page: u32,
        page_size: u32,
        sort: &str,
        data_dir: &Path,
    ) -> Result<MemePage, String> {
        let conn = self.conn.lock();
        let page = page.max(1);
        let page_size = page_size.clamp(1, 100);
        let offset = ((page - 1) * page_size) as i64;

        let mut where_clauses: Vec<String> = Vec::new();
        let mut sql_params: Vec<rusqlite::types::Value> = Vec::new();

        let trimmed_q = query.trim();
        if !trimmed_q.is_empty() {
            let fts_expr = build_fts_query(trimmed_q);
            let like_pat = format!("%{}%", trimmed_q.to_lowercase());
            let (q_py_full, q_py_init) = extract_pinyin(trimmed_q);

            if !fts_expr.is_empty() {
                where_clauses.push(
                    "(m.id IN (SELECT rowid FROM memes_fts WHERE memes_fts MATCH ?)
                      OR LOWER(m.alias) LIKE ?
                      OR m.pinyin_full LIKE ?
                      OR m.pinyin_initials LIKE ?
                      OR EXISTS (
                          SELECT 1 FROM meme_tags mt2
                          INNER JOIN tags t2 ON t2.id = mt2.tag_id
                          WHERE mt2.meme_id = m.id
                            AND (LOWER(t2.name) LIKE ? OR t2.pinyin_full LIKE ? OR t2.pinyin_initials LIKE ?)
                      ))"
                        .to_string(),
                );
                sql_params.push(fts_expr.into());
                sql_params.push(like_pat.clone().into());
                sql_params.push(format!("%{q_py_full}%").into());
                sql_params.push(format!("%{q_py_init}%").into());
                sql_params.push(like_pat.into());
                sql_params.push(format!("%{q_py_full}%").into());
                sql_params.push(format!("%{q_py_init}%").into());
            } else {
                where_clauses.push("LOWER(m.alias) LIKE ?".to_string());
                sql_params.push(like_pat.into());
            }
        }

        let clean_tags: Vec<String> = tags
            .iter()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();

        if !clean_tags.is_empty() {
            let placeholders = vec!["?"; clean_tags.len()].join(", ");
            where_clauses.push(format!(
                "m.id IN (
                    SELECT mt.meme_id
                    FROM meme_tags mt
                    INNER JOIN tags t ON t.id = mt.tag_id
                    WHERE t.name IN ({placeholders})
                    GROUP BY mt.meme_id
                    HAVING COUNT(DISTINCT t.id) = ?
                )"
            ));
            for t in &clean_tags {
                sql_params.push(t.clone().into());
            }
            sql_params.push((clean_tags.len() as i64).into());
        }

        let where_sql = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };

        let order_sql = match sort {
            "frequent" => "ORDER BY m.use_count DESC, COALESCE(m.last_used_at, 0) DESC, m.id DESC",
            "recent" => "ORDER BY COALESCE(m.last_used_at, m.created_at) DESC, m.id DESC",
            _ => "ORDER BY m.created_at DESC, m.id DESC",
        };

        let count_sql = format!("SELECT COUNT(*) FROM memes m {where_sql}");
        let total: i64 = conn
            .query_row(
                &count_sql,
                rusqlite::params_from_iter(sql_params.iter()),
                |row| row.get(0),
            )
            .map_err(|e| format!("查询总数失败: {e}"))?;

        let list_sql = format!(
            "SELECT m.id, m.sha256, m.alias, m.ext, m.mime, m.width, m.height,
                    m.file_size, m.is_animated, m.original_path, m.thumb_path,
                    m.use_count, m.last_used_at, m.created_at
             FROM memes m
             {where_sql}
             {order_sql}
             LIMIT ? OFFSET ?"
        );

        let mut list_params = sql_params;
        list_params.push((page_size as i64).into());
        list_params.push(offset.into());

        let mut stmt = conn.prepare(&list_sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(list_params.iter()), |row| {
                let rel_orig: String = row.get(9)?;
                let rel_thumb: String = row.get(10)?;
                Ok(MemeItem {
                    id: row.get(0)?,
                    sha256: row.get(1)?,
                    alias: row.get(2)?,
                    tags: Vec::new(),
                    ext: row.get(3)?,
                    mime: row.get(4)?,
                    width: row.get::<_, i64>(5)? as u32,
                    height: row.get::<_, i64>(6)? as u32,
                    file_size: row.get::<_, i64>(7)? as u64,
                    is_animated: row.get::<_, i64>(8)? != 0,
                    original_path: data_dir.join(rel_orig).to_string_lossy().to_string(),
                    thumb_path: data_dir.join(rel_thumb).to_string_lossy().to_string(),
                    use_count: row.get(11)?,
                    last_used_at: row.get(12)?,
                    created_at: row.get(13)?,
                })
            })
            .map_err(|e| e.to_string())?;

        let mut items = Vec::new();
        for r in rows {
            let mut item = r.map_err(|e| e.to_string())?;
            item.tags = get_tags_for_meme_conn(&conn, item.id)?;
            items.push(item);
        }

        let total_pages = if total == 0 {
            1
        } else {
            ((total as u64 + page_size as u64 - 1) / page_size as u64) as u32
        };

        Ok(MemePage {
            items,
            total,
            page,
            page_size,
            total_pages,
        })
    }

    pub fn get_settings(&self) -> Result<AppSettings, String> {
        let conn = self.conn.lock();
        let mut settings = AppSettings::default();
        let mut stmt = conn
            .prepare("SELECT key, value FROM settings")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;

        for row in rows {
            let (k, v) = row.map_err(|e| e.to_string())?;
            match k.as_str() {
                "toggle_shortcut" => settings.toggle_shortcut = v,
                "quick_capture_shortcut" => settings.quick_capture_shortcut = v,
                "auto_send" => settings.auto_send = v == "true",
                "qq_dock_enabled" => settings.qq_dock_enabled = v == "true",
                "page_size" => {
                    if let Ok(n) = v.parse::<u32>() {
                        settings.page_size = n.clamp(8, 60);
                    }
                }
                "theme" => settings.theme = v,
                "paste_delay_ms" => {
                    if let Ok(n) = v.parse::<u64>() {
                        settings.paste_delay_ms = n.clamp(30, 1000);
                    }
                }
                _ => {}
            }
        }
        Ok(settings)
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<(), String> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let pairs = [
            ("toggle_shortcut", settings.toggle_shortcut.clone()),
            (
                "quick_capture_shortcut",
                settings.quick_capture_shortcut.clone(),
            ),
            ("auto_send", settings.auto_send.to_string()),
            ("qq_dock_enabled", settings.qq_dock_enabled.to_string()),
            ("page_size", settings.page_size.clamp(8, 60).to_string()),
            ("theme", settings.theme.clone()),
            (
                "paste_delay_ms",
                settings.paste_delay_ms.clamp(30, 1000).to_string(),
            ),
        ];

        for (k, v) in pairs {
            tx.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![k, v],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn build_fts_query(raw: &str) -> String {
    let tokens: Vec<String> = raw
        .split_whitespace()
        .map(|tok| {
            let cleaned: String = tok
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '_' || !c.is_ascii_punctuation())
                .collect();
            cleaned
        })
        .filter(|s| !s.is_empty())
        .map(|s| format!("\"{s}\"*"))
        .collect();
    tokens.join(" AND ")
}

fn get_tags_for_meme_conn(conn: &Connection, meme_id: i64) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT t.name
             FROM tags t
             INNER JOIN meme_tags mt ON mt.tag_id = t.id
             WHERE mt.meme_id = ?1
             ORDER BY t.name ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![meme_id], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

fn get_meme_by_id_conn(
    conn: &Connection,
    id: i64,
    data_dir: &Path,
) -> Result<Option<MemeItem>, String> {
    let mut item: Option<MemeItem> = conn
        .query_row(
            "SELECT id, sha256, alias, ext, mime, width, height,
                    file_size, is_animated, original_path, thumb_path,
                    use_count, last_used_at, created_at
             FROM memes WHERE id = ?1",
            params![id],
            |row| {
                let rel_orig: String = row.get(9)?;
                let rel_thumb: String = row.get(10)?;
                Ok(MemeItem {
                    id: row.get(0)?,
                    sha256: row.get(1)?,
                    alias: row.get(2)?,
                    tags: Vec::new(),
                    ext: row.get(3)?,
                    mime: row.get(4)?,
                    width: row.get::<_, i64>(5)? as u32,
                    height: row.get::<_, i64>(6)? as u32,
                    file_size: row.get::<_, i64>(7)? as u64,
                    is_animated: row.get::<_, i64>(8)? != 0,
                    original_path: data_dir.join(rel_orig).to_string_lossy().to_string(),
                    thumb_path: data_dir.join(rel_thumb).to_string_lossy().to_string(),
                    use_count: row.get(11)?,
                    last_used_at: row.get(12)?,
                    created_at: row.get(13)?,
                })
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if let Some(ref mut m) = item {
        m.tags = get_tags_for_meme_conn(conn, m.id)?;
    }
    Ok(item)
}

fn set_meme_tags_tx(
    tx: &rusqlite::Transaction<'_>,
    meme_id: i64,
    tags: &[String],
) -> Result<(), String> {
    tx.execute("DELETE FROM meme_tags WHERE meme_id = ?1", params![meme_id])
        .map_err(|e| e.to_string())?;

    for raw_tag in tags {
        let name = raw_tag.trim();
        if name.is_empty() {
            continue;
        }
        let (py_full, py_init) = extract_pinyin(name);
        tx.execute(
            "INSERT INTO tags (name, pinyin_full, pinyin_initials) VALUES (?1, ?2, ?3)
             ON CONFLICT(name) DO NOTHING",
            params![name, py_full, py_init],
        )
        .map_err(|e| e.to_string())?;

        let tag_id: i64 = tx
            .query_row("SELECT id FROM tags WHERE name = ?1", params![name], |row| {
                row.get(0)
            })
            .map_err(|e| e.to_string())?;

        tx.execute(
            "INSERT OR IGNORE INTO meme_tags (meme_id, tag_id) VALUES (?1, ?2)",
            params![meme_id, tag_id],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.execute(
        "DELETE FROM tags WHERE id NOT IN (SELECT DISTINCT tag_id FROM meme_tags)",
        [],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn sync_fts_tx(tx: &rusqlite::Transaction<'_>, meme_id: i64) -> Result<(), String> {
    let (alias, py_full, py_init): (String, String, String) = tx
        .query_row(
            "SELECT alias, pinyin_full, pinyin_initials FROM memes WHERE id = ?1",
            params![meme_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| e.to_string())?;

    let mut stmt = tx
        .prepare(
            "SELECT t.name, t.pinyin_full, t.pinyin_initials
             FROM tags t
             INNER JOIN meme_tags mt ON mt.tag_id = t.id
             WHERE mt.meme_id = ?1",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![meme_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut tag_tokens = Vec::new();
    for r in rows {
        let (name, t_full, t_init) = r.map_err(|e| e.to_string())?;
        tag_tokens.push(name);
        if !t_full.is_empty() {
            tag_tokens.push(t_full);
        }
        if !t_init.is_empty() {
            tag_tokens.push(t_init);
        }
    }
    let tags_text = tag_tokens.join(" ");

    tx.execute("DELETE FROM memes_fts WHERE rowid = ?1", params![meme_id])
        .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO memes_fts (rowid, alias, pinyin_full, pinyin_initials, tags_text)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![meme_id, alias, py_full, py_init, tags_text],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_db_fts_pinyin_and_multi_tag_search() {
        let dir = tempdir().unwrap();
        let db = Database::open(&dir.path().join("test.db")).unwrap();

        let m1 = db
            .insert_meme(
                NewMemeRecord {
                    sha256: "aaa111",
                    alias: "熊猫头震惊",
                    ext: "gif",
                    mime: "image/gif",
                    width: 200,
                    height: 200,
                    file_size: 12345,
                    is_animated: true,
                    original_path: "originals/aa/aaa111.gif",
                    thumb_path: "thumbs/aa/aaa111.webp",
                    tags: &["搞笑".to_string(), "震惊".to_string()],
                },
                dir.path(),
            )
            .unwrap();

        let _m2 = db
            .insert_meme(
                NewMemeRecord {
                    sha256: "bbb222",
                    alias: "猫猫比心",
                    ext: "png",
                    mime: "image/png",
                    width: 150,
                    height: 150,
                    file_size: 4567,
                    is_animated: false,
                    original_path: "originals/bb/bbb222.png",
                    thumb_path: "thumbs/bb/bbb222.webp",
                    tags: &["可爱".to_string(), "搞笑".to_string()],
                },
                dir.path(),
            )
            .unwrap();

        // Pinyin initials search
        let res = db.query_memes("xmt", &[], 1, 20, "newest", dir.path()).unwrap();
        assert_eq!(res.total, 1);
        assert_eq!(res.items[0].id, m1.id);

        // Full pinyin search
        let res2 = db
            .query_memes("maomao", &[], 1, 20, "newest", dir.path())
            .unwrap();
        assert_eq!(res2.total, 1);
        assert_eq!(res2.items[0].alias, "猫猫比心");

        // Multi-tag AND filter
        let res3 = db
            .query_memes(
                "",
                &["搞笑".to_string(), "震惊".to_string()],
                1,
                20,
                "newest",
                dir.path(),
            )
            .unwrap();
        assert_eq!(res3.total, 1);
        assert_eq!(res3.items[0].id, m1.id);

        // Update alias & tags
        let updated = db
            .update_meme_meta(m1.id, "狗头保命", &["狗头".to_string()], dir.path())
            .unwrap();
        assert_eq!(updated.alias, "狗头保命");
        assert_eq!(updated.tags, vec!["狗头".to_string()]);

        let res4 = db.query_memes("gtbm", &[], 1, 20, "newest", dir.path()).unwrap();
        assert_eq!(res4.total, 1);

        // Settings roundtrip
        let mut settings = db.get_settings().unwrap();
        assert!(!settings.auto_send);
        settings.auto_send = true;
        settings.qq_dock_enabled = true;
        db.save_settings(&settings).unwrap();
        let loaded = db.get_settings().unwrap();
        assert!(loaded.auto_send);
        assert!(loaded.qq_dock_enabled);
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemeItem {
    pub id: i64,
    pub sha256: String,
    pub alias: String,
    pub tags: Vec<String>,
    pub ext: String,
    pub mime: String,
    pub width: u32,
    pub height: u32,
    pub file_size: u64,
    pub is_animated: bool,
    pub original_path: String,
    pub thumb_path: String,
    pub use_count: i64,
    pub last_used_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemePage {
    pub items: Vec<MemeItem>,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
    pub total_pages: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagInfo {
    pub id: i64,
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub toggle_shortcut: String,
    pub quick_capture_shortcut: String,
    pub auto_send: bool,
    pub qq_dock_enabled: bool,
    pub page_size: u32,
    pub theme: String,
    pub paste_delay_ms: u64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            toggle_shortcut: "Alt+E".to_string(),
            quick_capture_shortcut: "Alt+Shift+E".to_string(),
            auto_send: false,
            qq_dock_enabled: false,
            page_size: 20,
            theme: "system".to_string(),
            paste_delay_ms: 120,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub imported: usize,
    pub duplicates: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
    pub added_items: Vec<MemeItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanCandidate {
    pub path: String,
    pub sha256: String,
    pub ext: String,
    pub mime: String,
    pub width: u32,
    pub height: u32,
    pub file_size: u64,
    pub is_animated: bool,
    pub already_imported: bool,
    pub suggested_alias: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QqDirInfo {
    pub path: String,
    pub label: String,
    pub file_count_hint: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewPayload {
    pub id: i64,
    pub alias: String,
    pub tags: Vec<String>,
    pub original_path: String,
    pub ext: String,
    pub width: u32,
    pub height: u32,
    pub file_size: u64,
    pub is_animated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformStatus {
    pub os: String,
    pub accessibility_granted: bool,
    pub data_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteResult {
    pub copied: bool,
    pub pasted: bool,
    pub needs_accessibility: bool,
    pub message: String,
}

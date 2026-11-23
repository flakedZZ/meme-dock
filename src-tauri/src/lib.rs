pub mod clipboard;
pub mod db;
pub mod image_util;
pub mod models;
pub mod paste;
pub mod qq_dock;
pub mod qq_scanner;
pub mod storage;

use crate::clipboard::{copy_file_to_clipboard, read_clipboard_images, ClipboardImageInput};
use crate::models::{
    AppSettings, ImportResult, MemeItem, MemePage, PasteResult, PlatformStatus, PreviewPayload,
    QqDirInfo, ScanCandidate, TagInfo,
};
use crate::paste::{
    check_accessibility_permission, open_accessibility_settings, simulate_paste,
};
use crate::storage::{ImportOutcome, MemeStore};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, LogicalPosition, Manager, Position, State, WebviewWindow, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub struct AppState {
    pub store: MemeStore,
    pub qq_dock_enabled: Arc<AtomicBool>,
    pub suspend_blur_hide: Arc<AtomicBool>,
}

fn hide_preview_window(app: &AppHandle) {
    if let Some(preview) = app.get_webview_window("preview") {
        let _ = preview.hide();
    }
}

fn toggle_main_window(app: &AppHandle) {
    let Some(main) = app.get_webview_window("main") else {
        return;
    };
    let visible = main.is_visible().unwrap_or(false);
    if visible {
        hide_preview_window(app);
        let _ = main.hide();
    } else {
        let _ = main.show();
        let _ = main.unminimize();
        let _ = main.set_focus();
        let _ = main.emit("focus-search", ());
    }
}

fn register_shortcuts(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();

    let toggle_str = settings.toggle_shortcut.trim().to_string();
    if !toggle_str.is_empty() {
        gs.on_shortcut(toggle_str.as_str(), |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                toggle_main_window(app);
            }
        })
        .map_err(|e| format!("注册召出快捷键 ({toggle_str}) 失败: {e}"))?;
    }

    let capture_str = settings.quick_capture_shortcut.trim().to_string();
    if !capture_str.is_empty() && capture_str != toggle_str {
        gs.on_shortcut(capture_str.as_str(), |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                let state = app.state::<AppState>();
                if let Ok(res) = run_clipboard_import(&state.store, &[]) {
                    let _ = app.emit("clipboard-imported", res);
                }
            }
        })
        .map_err(|e| format!("注册剪贴板快收快捷键 ({capture_str}) 失败: {e}"))?;
    }

    Ok(())
}

fn run_clipboard_import(store: &MemeStore, tags: &[String]) -> Result<ImportResult, String> {
    let inputs = read_clipboard_images()?;
    if inputs.is_empty() {
        return Ok(ImportResult {
            imported: 0,
            duplicates: 0,
            skipped: 0,
            errors: vec!["剪贴板中未检测到图片或图片文件".to_string()],
            added_items: Vec::new(),
        });
    }

    let mut result = ImportResult {
        imported: 0,
        duplicates: 0,
        skipped: 0,
        errors: Vec::new(),
        added_items: Vec::new(),
    };

    for input in inputs {
        match input {
            ClipboardImageInput::FilePath(p) => {
                let sub = store.import_paths(&[p], tags);
                result.imported += sub.imported;
                result.duplicates += sub.duplicates;
                result.skipped += sub.skipped;
                result.errors.extend(sub.errors);
                result.added_items.extend(sub.added_items);
            }
            ClipboardImageInput::RawBytes { bytes, alias } => {
                match store.import_bytes(&bytes, &alias, tags) {
                    Ok(ImportOutcome::Imported(item)) => {
                        result.imported += 1;
                        result.added_items.push(item);
                    }
                    Ok(ImportOutcome::Duplicate) => result.duplicates += 1,
                    Ok(ImportOutcome::Skipped) => result.skipped += 1,
                    Err(e) => result.errors.push(e),
                }
            }
        }
    }

    Ok(result)
}

#[tauri::command]
fn query_memes(
    state: State<'_, AppState>,
    query: String,
    tags: Vec<String>,
    page: u32,
    page_size: u32,
    sort: String,
) -> Result<MemePage, String> {
    state
        .store
        .db
        .query_memes(&query, &tags, page, page_size, &sort, &state.store.data_dir)
}

#[tauri::command]
fn list_tags(state: State<'_, AppState>) -> Result<Vec<TagInfo>, String> {
    state.store.db.list_tags()
}

#[tauri::command]
fn update_meme_meta(
    state: State<'_, AppState>,
    id: i64,
    alias: String,
    tags: Vec<String>,
) -> Result<MemeItem, String> {
    state
        .store
        .db
        .update_meme_meta(id, &alias, &tags, &state.store.data_dir)
}

#[tauri::command]
fn delete_meme(state: State<'_, AppState>, id: i64) -> Result<bool, String> {
    state.store.delete_meme_with_files(id)
}

#[tauri::command]
fn import_paths(
    state: State<'_, AppState>,
    paths: Vec<String>,
    tags: Vec<String>,
) -> Result<ImportResult, String> {
    let pb: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    Ok(state.store.import_paths(&pb, &tags))
}

#[tauri::command]
fn import_from_clipboard(
    state: State<'_, AppState>,
    tags: Vec<String>,
) -> Result<ImportResult, String> {
    run_clipboard_import(&state.store, &tags)
}

#[tauri::command]
fn discover_qq_dirs() -> Vec<QqDirInfo> {
    qq_scanner::discover_qq_dirs()
}

#[tauri::command]
fn scan_qq_dir(
    state: State<'_, AppState>,
    dir_path: String,
    max_items: Option<usize>,
) -> Result<Vec<ScanCandidate>, String> {
    let p = PathBuf::from(dir_path);
    qq_scanner::scan_directory_for_memes(&p, &state.store.db, max_items.unwrap_or(240))
}

#[tauri::command]
fn paste_meme(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    keep_open: bool,
) -> Result<PasteResult, String> {
    let orig_path = state.store.resolve_original_path(id)?;
    let _ = state.store.db.record_use(id);

    copy_file_to_clipboard(&orig_path)?;

    let settings = state.store.db.get_settings().unwrap_or_default();
    hide_preview_window(&app);

    let has_access = check_accessibility_permission();
    if !has_access {
        if !keep_open {
            if let Some(main) = app.get_webview_window("main") {
                let _ = main.hide();
            }
        }
        return Ok(PasteResult {
            copied: true,
            pasted: false,
            needs_accessibility: true,
            message: "已复制到剪贴板；如需自动粘贴，请在系统设置中授予 MemeDock 辅助功能权限"
                .to_string(),
        });
    }

    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }

    let paste_res = simulate_paste(settings.auto_send, settings.paste_delay_ms);

    if keep_open {
        if let Some(main) = app.get_webview_window("main") {
            let _ = main.show();
            let _ = main.set_focus();
        }
    }

    match paste_res {
        Ok(()) => Ok(PasteResult {
            copied: true,
            pasted: true,
            needs_accessibility: false,
            message: if settings.auto_send {
                "已粘贴并发送".to_string()
            } else {
                "已粘贴到输入框".to_string()
            },
        }),
        Err(e) => Ok(PasteResult {
            copied: true,
            pasted: false,
            needs_accessibility: false,
            message: format!("已复制到剪贴板（自动粘贴未触发: {e}）"),
        }),
    }
}

#[tauri::command]
fn show_preview(app: AppHandle, state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let Some(item) = state.store.db.get_meme_by_id(id, &state.store.data_dir)? else {
        return Ok(());
    };
    let Some(main) = app.get_webview_window("main") else {
        return Ok(());
    };
    let Some(preview) = app.get_webview_window("preview") else {
        return Ok(());
    };

    let scale = main.scale_factor().unwrap_or(1.0);
    let main_pos = main
        .outer_position()
        .map(|p| p.to_logical::<f64>(scale))
        .unwrap_or(LogicalPosition::new(100.0, 100.0));
    let main_size = main
        .outer_size()
        .map(|s| s.to_logical::<f64>(scale))
        .unwrap_or(tauri::LogicalSize::new(540.0, 540.0));

    let preview_w = 288.0;
    let preview_h = 336.0;
    let gap = 10.0;

    let mut target_x = main_pos.x + main_size.width + gap;
    let mut target_y = main_pos.y + (main_size.height - preview_h) * 0.5;

    if let Ok(Some(monitor)) = main.current_monitor() {
        let mon_pos = monitor.position().to_logical::<f64>(scale);
        let mon_size = monitor.size().to_logical::<f64>(scale);
        let right_edge = mon_pos.x + mon_size.width;
        if target_x + preview_w > right_edge - 8.0 {
            target_x = (main_pos.x - preview_w - gap).max(mon_pos.x + 8.0);
        }
        let max_y = mon_pos.y + mon_size.height - preview_h - 8.0;
        target_y = target_y.clamp(mon_pos.y + 8.0, max_y.max(mon_pos.y + 8.0));
    }

    let _ = preview.set_position(Position::Logical(LogicalPosition::new(target_x, target_y)));
    let payload = PreviewPayload {
        id: item.id,
        alias: item.alias,
        tags: item.tags,
        original_path: item.original_path,
        ext: item.ext,
        width: item.width,
        height: item.height,
        file_size: item.file_size,
        is_animated: item.is_animated,
    };
    let _ = preview.emit("preview-update", payload);
    let _ = preview.show();
    Ok(())
}

#[tauri::command]
fn hide_preview(app: AppHandle) {
    hide_preview_window(&app);
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    state.store.db.get_settings()
}

#[tauri::command]
fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    register_shortcuts(&app, &settings)?;
    state.store.db.save_settings(&settings)?;
    state
        .qq_dock_enabled
        .store(settings.qq_dock_enabled, Ordering::Relaxed);
    let _ = app.emit("settings-updated", settings.clone());
    Ok(settings)
}

#[tauri::command]
fn get_platform_status(state: State<'_, AppState>) -> PlatformStatus {
    PlatformStatus {
        os: std::env::consts::OS.to_string(),
        accessibility_granted: check_accessibility_permission(),
        data_dir: state.store.data_dir.to_string_lossy().to_string(),
    }
}

#[tauri::command]
fn open_accessibility() -> Result<(), String> {
    open_accessibility_settings()
}

#[tauri::command]
fn set_suspend_blur_hide(state: State<'_, AppState>, suspend: bool) {
    state.suspend_blur_hide.store(suspend, Ordering::Relaxed);
}

#[tauri::command]
fn open_main_from_dock(app: AppHandle) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.unminimize();
        let _ = main.set_focus();
        let _ = main.emit("focus-search", ());
    }
}

#[tauri::command]
fn hide_main_window(app: AppHandle) {
    hide_preview_window(&app);
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, "toggle", "显示 / 隐藏 MemeDock", true, None::<&str>)?;
    let clip_item = MenuItem::with_id(app, "clip_import", "快速导入剪贴板表情", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出 MemeDock", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &clip_item, &quit_item])?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("MemeDock 表情包工具")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => toggle_main_window(app),
            "clip_import" => {
                let state = app.state::<AppState>();
                if let Ok(res) = run_clipboard_import(&state.store, &[]) {
                    let _ = app.emit("clipboard-imported", res);
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn attach_main_window_events(main: WebviewWindow, suspend_blur_hide: Arc<AtomicBool>) {
    let app_handle = main.app_handle().clone();
    main.on_window_event(move |event| match event {
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            hide_preview_window(&app_handle);
            if let Some(w) = app_handle.get_webview_window("main") {
                let _ = w.hide();
            }
        }
        WindowEvent::Focused(false) => {
            if !suspend_blur_hide.load(Ordering::Relaxed) {
                hide_preview_window(&app_handle);
                if let Some(w) = app_handle.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
        }
        _ => {}
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("无法获取应用数据目录: {e}"))?;
            let store = MemeStore::init(data_dir)?;
            let settings = store.db.get_settings().unwrap_or_default();

            let qq_dock_enabled = Arc::new(AtomicBool::new(settings.qq_dock_enabled));
            let suspend_blur_hide = Arc::new(AtomicBool::new(false));

            app.manage(AppState {
                store,
                qq_dock_enabled: qq_dock_enabled.clone(),
                suspend_blur_hide: suspend_blur_hide.clone(),
            });

            setup_tray(app)?;
            let _ = register_shortcuts(app.handle(), &settings);
            qq_dock::spawn_qq_dock_watcher(app.handle().clone(), qq_dock_enabled);

            if let Some(main) = app.get_webview_window("main") {
                attach_main_window_events(main, suspend_blur_hide);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            query_memes,
            list_tags,
            update_meme_meta,
            delete_meme,
            import_paths,
            import_from_clipboard,
            discover_qq_dirs,
            scan_qq_dir,
            paste_meme,
            show_preview,
            hide_preview,
            get_settings,
            save_settings,
            get_platform_status,
            open_accessibility,
            set_suspend_blur_hide,
            open_main_from_dock,
            hide_main_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MemeDock");
}

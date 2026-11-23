use crate::AppState;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;

pub fn check_on_startup(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = check(app).await {
            eprintln!("MemeDock 更新检查失败：{error}");
        }
    });
}

async fn check(app: AppHandle) -> tauri_plugin_updater::Result<()> {
    let updater = app
        .updater_builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    let Some(mut update) = updater.check().await? else {
        return Ok(());
    };

    update.timeout = Some(Duration::from_secs(300));
    let mut prompt = app
        .dialog()
        .message(format!(
            "发现 MemeDock {}，当前版本为 {}。现在下载安装？安装时应用会退出。",
            update.version, update.current_version
        ))
        .title("MemeDock 更新")
        .buttons(MessageDialogButtons::OkCancelCustom(
            "下载并安装".into(),
            "稍后".into(),
        ));

    if let Some(main) = app.get_webview_window("main") {
        if main.is_visible().unwrap_or(false) {
            prompt = prompt.parent(&main);
        }
    }
    app.state::<AppState>()
        .update_prompt_open
        .store(true, Ordering::Relaxed);
    prompt.show(move |confirmed| {
        app.state::<AppState>()
            .update_prompt_open
            .store(false, Ordering::Relaxed);
        if !confirmed {
            return;
        }
        tauri::async_runtime::spawn(async move {
            match update.download_and_install(|_, _| {}, || {}).await {
                Ok(()) => app.restart(),
                Err(error) => app
                    .dialog()
                    .message(format!("更新失败，可以在下次启动时重试。\n{error}"))
                    .title("MemeDock 更新")
                    .kind(MessageDialogKind::Error)
                    .show(|_| {}),
            }
        });
    });

    Ok(())
}

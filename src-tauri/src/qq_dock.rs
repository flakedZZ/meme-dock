use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, LogicalPosition, Manager, Position};

#[derive(Debug, Clone, Copy)]
pub struct WindowRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Start low-frequency QQ foreground window watcher for the optional edge dock trigger button.
pub fn spawn_qq_dock_watcher(app: AppHandle, enabled_flag: Arc<AtomicBool>) {
    thread::spawn(move || {
        let mut last_visible = false;
        let mut last_pos: Option<(i32, i32)> = None;

        loop {
            if !enabled_flag.load(Ordering::Relaxed) {
                if last_visible {
                    if let Some(win) = app.get_webview_window("qq-dock") {
                        let _ = win.hide();
                    }
                    last_visible = false;
                    last_pos = None;
                }
                thread::sleep(Duration::from_millis(900));
                continue;
            }

            // Hide dock button while main MemeDock panel is open
            let main_visible = app
                .get_webview_window("main")
                .and_then(|w| w.is_visible().ok())
                .unwrap_or(false);

            if main_visible {
                if last_visible {
                    if let Some(win) = app.get_webview_window("qq-dock") {
                        let _ = win.hide();
                    }
                    last_visible = false;
                }
                thread::sleep(Duration::from_millis(450));
                continue;
            }

            if let Some(rect) = detect_foreground_qq_window() {
                let target_x = (rect.x + rect.width - 2.0).round() as i32;
                let target_y = (rect.y + rect.height * 0.42).round() as i32;

                if let Some(win) = app.get_webview_window("qq-dock") {
                    if last_pos != Some((target_x, target_y)) {
                        let _ = win.set_position(Position::Logical(LogicalPosition::new(
                            target_x as f64,
                            target_y as f64,
                        )));
                        last_pos = Some((target_x, target_y));
                    }
                    if !last_visible {
                        let _ = win.show();
                        last_visible = true;
                    }
                }
            } else if last_visible {
                if let Some(win) = app.get_webview_window("qq-dock") {
                    let _ = win.hide();
                }
                last_visible = false;
                last_pos = None;
            }

            thread::sleep(Duration::from_millis(500));
        }
    });
}

fn detect_foreground_qq_window() -> Option<WindowRect> {
    #[cfg(target_os = "macos")]
    {
        macos::foreground_qq_rect()
    }

    #[cfg(target_os = "windows")]
    {
        windows::foreground_qq_rect()
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::WindowRect;
    use core_foundation::array::CFArray;
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics::display::{
        kCGNullWindowID, kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly,
        CGWindowListCopyWindowInfo,
    };
    use objc2::msg_send;
    use objc2_app_kit::NSWorkspace;

    unsafe fn dict_f64(dict: &CFDictionary, key: &CFString) -> f64 {
        dict.find(key.as_CFTypeRef())
            .and_then(|val_ptr| {
                let cf = CFType::wrap_under_get_rule(*val_ptr);
                cf.downcast::<CFNumber>().and_then(|n| n.to_f64())
            })
            .unwrap_or(0.0)
    }

    pub fn foreground_qq_rect() -> Option<WindowRect> {
        let qq_pid: i32 = unsafe {
            let ws = NSWorkspace::sharedWorkspace();
            let front = ws.frontmostApplication()?;
            let bundle_id = front
                .bundleIdentifier()
                .map(|s| s.to_string())
                .unwrap_or_default();
            if bundle_id != "com.tencent.qq" {
                return None;
            }
            let pid: i32 = msg_send![&front, processIdentifier];
            pid
        };

        unsafe {
            let list_ref = CGWindowListCopyWindowInfo(
                kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
                kCGNullWindowID,
            );
            if list_ref.is_null() {
                return None;
            }
            let arr: CFArray<CFDictionary<CFString, CFType>> =
                CFArray::wrap_under_create_rule(list_ref);

            let key_pid = CFString::new("kCGWindowOwnerPID");
            let key_layer = CFString::new("kCGWindowLayer");
            let key_bounds = CFString::new("kCGWindowBounds");
            let key_x = CFString::new("X");
            let key_y = CFString::new("Y");
            let key_w = CFString::new("Width");
            let key_h = CFString::new("Height");

            let mut best: Option<WindowRect> = None;
            for dict in arr.iter() {
                let pid = dict
                    .find(&key_pid)
                    .and_then(|v| v.downcast::<CFNumber>())
                    .and_then(|n| n.to_i32())
                    .unwrap_or(-1);
                if pid != qq_pid {
                    continue;
                }
                let layer = dict
                    .find(&key_layer)
                    .and_then(|v| v.downcast::<CFNumber>())
                    .and_then(|n| n.to_i32())
                    .unwrap_or(1);
                if layer != 0 {
                    continue;
                }
                if let Some(bounds_val) = dict.find(&key_bounds) {
                    if let Some(bdict) = bounds_val.downcast::<CFDictionary>() {
                        let x = dict_f64(&bdict, &key_x);
                        let y = dict_f64(&bdict, &key_y);
                        let w = dict_f64(&bdict, &key_w);
                        let h = dict_f64(&bdict, &key_h);

                        if w >= 320.0 && h >= 260.0 {
                            let cand = WindowRect {
                                x,
                                y,
                                width: w,
                                height: h,
                            };
                            if best
                                .map(|b| cand.width * cand.height > b.width * b.height)
                                .unwrap_or(true)
                            {
                                best = Some(cand);
                            }
                        }
                    }
                }
            }
            best
        }
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::WindowRect;
    use windows_sys::Win32::Foundation::{CloseHandle, RECT};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId,
    };

    pub fn foreground_qq_rect() -> Option<WindowRect> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return None;
            }
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == 0 {
                return None;
            }

            let proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if proc.is_null() {
                return None;
            }
            let mut buf = [0u16; 512];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(proc, 0, buf.as_mut_ptr(), &mut len);
            CloseHandle(proc);
            if ok == 0 {
                return None;
            }
            let path = String::from_utf16_lossy(&buf[..len as usize]).to_lowercase();
            if !path.ends_with("\\qq.exe") {
                return None;
            }

            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            if GetWindowRect(hwnd, &mut rect) == 0 {
                return None;
            }
            let width = (rect.right - rect.left) as f64;
            let height = (rect.bottom - rect.top) as f64;
            if width < 320.0 || height < 260.0 {
                return None;
            }

            Some(WindowRect {
                x: rect.left as f64,
                y: rect.top as f64,
                width,
                height,
            })
        }
    }
}

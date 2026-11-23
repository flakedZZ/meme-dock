use std::thread;
use std::time::Duration;

/// Check whether OS permits simulated keyboard input (macOS Accessibility).
pub fn check_accessibility_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXIsProcessTrusted() -> bool;
        }
        unsafe { AXIsProcessTrusted() }
    }

    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Open macOS System Settings -> Privacy & Security -> Accessibility.
pub fn open_accessibility_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn()
            .map_err(|e| format!("无法打开辅助功能设置: {e}"))?;
    }
    Ok(())
}

/// Simulate Ctrl+V (Windows) or Cmd+V (macOS), and optionally Enter if `auto_send` is enabled.
pub(crate) fn simulate_paste(
    auto_send: bool,
    delay_ms: u64,
    #[cfg(target_os = "windows")] target: WindowsPasteTarget,
) -> Result<(), String> {
    if delay_ms > 0 {
        thread::sleep(Duration::from_millis(delay_ms));
    }

    #[cfg(target_os = "macos")]
    {
        macos::send_cmd_v(auto_send)
    }

    #[cfg(target_os = "windows")]
    {
        windows::send_ctrl_v(target, auto_send)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = auto_send;
        Err("当前系统不支持自动模拟粘贴".to_string())
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use std::thread;
    use std::time::Duration;

    const K_VK_ANSI_V: u16 = 0x09;
    const K_VK_RETURN: u16 = 0x24;

    pub fn send_cmd_v(auto_send: bool) -> Result<(), String> {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| "创建 macOS CGEventSource 失败".to_string())?;

        let key_down = CGEvent::new_keyboard_event(source.clone(), K_VK_ANSI_V, true)
            .map_err(|_| "创建 Cmd+V 按下事件失败".to_string())?;
        key_down.set_flags(CGEventFlags::CGEventFlagCommand);
        key_down.post(CGEventTapLocation::HID);

        thread::sleep(Duration::from_millis(18));

        let key_up = CGEvent::new_keyboard_event(source.clone(), K_VK_ANSI_V, false)
            .map_err(|_| "创建 Cmd+V 抬起事件失败".to_string())?;
        key_up.set_flags(CGEventFlags::CGEventFlagCommand);
        key_up.post(CGEventTapLocation::HID);

        if auto_send {
            thread::sleep(Duration::from_millis(90));
            let ret_down = CGEvent::new_keyboard_event(source.clone(), K_VK_RETURN, true)
                .map_err(|_| "创建回车按下事件失败".to_string())?;
            ret_down.post(CGEventTapLocation::HID);
            thread::sleep(Duration::from_millis(18));
            let ret_up = CGEvent::new_keyboard_event(source, K_VK_RETURN, false)
                .map_err(|_| "创建回车抬起事件失败".to_string())?;
            ret_up.post(CGEventTapLocation::HID);
        }

        Ok(())
    }
}

#[cfg(target_os = "windows")]
pub(crate) use windows::WindowsPasteTarget;

#[cfg(target_os = "windows")]
mod windows {
    use std::thread;
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
        VK_CONTROL, VK_MENU, VK_RETURN, VK_SHIFT,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
    };

    #[derive(Clone, Copy)]
    pub(crate) struct WindowsPasteTarget {
        hwnd: usize,
        pid: u32,
    }

    impl WindowsPasteTarget {
        pub(crate) fn capture() -> Option<Self> {
            let hwnd = unsafe { GetForegroundWindow() };
            let mut pid = 0;
            if hwnd.is_null()
                || unsafe { GetWindowThreadProcessId(hwnd, &mut pid) } == 0
                || !crate::qq_dock::windows::is_qq_process(pid)
            {
                return None;
            }
            Some(Self {
                hwnd: hwnd as usize,
                pid,
            })
        }

        pub(crate) fn restore(self, panel: HWND) -> Result<(), String> {
            let hwnd = self.hwnd as HWND;
            let mut pid = 0;
            let foreground = unsafe { GetForegroundWindow() };
            if unsafe { GetWindowThreadProcessId(hwnd, &mut pid) } == 0 || pid != self.pid {
                return Err("原来的 QQ 窗口已关闭".to_string());
            }
            if foreground != hwnd
                && (foreground != panel || unsafe { SetForegroundWindow(hwnd) } == 0)
            {
                return Err("无法切回原来的 QQ 窗口".to_string());
            }
            Ok(())
        }

        pub(crate) fn wait_for_foreground(self) -> Result<(), String> {
            let deadline = Instant::now() + Duration::from_millis(250);
            loop {
                match self.verify_foreground() {
                    Ok(()) => return Ok(()),
                    Err(err) if Instant::now() >= deadline => return Err(err),
                    Err(_) => thread::sleep(Duration::from_millis(10)),
                }
            }
        }

        pub(crate) fn verify_foreground(self) -> Result<(), String> {
            let hwnd = self.hwnd as HWND;
            let mut pid = 0;
            if unsafe { GetForegroundWindow() } == hwnd
                && unsafe { GetWindowThreadProcessId(hwnd, &mut pid) } != 0
                && pid == self.pid
            {
                Ok(())
            } else {
                Err("原来的 QQ 窗口已失去焦点".to_string())
            }
        }
    }

    fn wait_for_modifiers() -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_millis(750);
        while [VK_SHIFT, VK_MENU, VK_CONTROL]
            .iter()
            .any(|&key| unsafe { GetAsyncKeyState(i32::from(key)) } < 0)
        {
            if Instant::now() >= deadline {
                return Err("请松开 Shift、Alt 或 Ctrl 后重试".to_string());
            }
            thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }

    const VK_V: u16 = 0x56;

    fn key_input(vk: u16, key_up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if key_up { KEYEVENTF_KEYUP } else { 0 },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn send_keys(inputs: &[INPUT], shortcut: &str) -> Result<(), String> {
        let sent = unsafe {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            )
        };
        if sent != inputs.len() as u32 {
            // Release any synthetically pressed key if Windows accepted only part of the sequence.
            let releases = &inputs[inputs.len() / 2..];
            if sent > 0 {
                unsafe {
                    SendInput(
                        releases.len() as u32,
                        releases.as_ptr(),
                        std::mem::size_of::<INPUT>() as i32,
                    );
                }
            }
            return Err(format!("Windows 未完整发送 {shortcut}"));
        }
        Ok(())
    }

    pub(super) fn send_ctrl_v(target: WindowsPasteTarget, auto_send: bool) -> Result<(), String> {
        wait_for_modifiers()?;
        target.verify_foreground()?;
        send_keys(
            &[
                key_input(VK_CONTROL, false),
                key_input(VK_V, false),
                key_input(VK_V, true),
                key_input(VK_CONTROL, true),
            ],
            "Ctrl+V",
        )?;

        if auto_send {
            thread::sleep(Duration::from_millis(90));
            wait_for_modifiers()?;
            target.verify_foreground()?;
            send_keys(
                &[key_input(VK_RETURN, false), key_input(VK_RETURN, true)],
                "Enter",
            )?;
        }
        Ok(())
    }
}

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
pub fn simulate_paste(auto_send: bool, delay_ms: u64) -> Result<(), String> {
    if delay_ms > 0 {
        thread::sleep(Duration::from_millis(delay_ms));
    }

    #[cfg(target_os = "macos")]
    {
        macos::send_cmd_v(auto_send)
    }

    #[cfg(target_os = "windows")]
    {
        windows::send_ctrl_v(auto_send)
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
mod windows {
    use std::thread;
    use std::time::Duration;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL,
        VK_RETURN,
    };

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

    pub fn send_ctrl_v(auto_send: bool) -> Result<(), String> {
        let inputs = [
            key_input(VK_CONTROL, false),
            key_input(VK_V, false),
            key_input(VK_V, true),
            key_input(VK_CONTROL, true),
        ];

        unsafe {
            let sent = SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            );
            if sent == 0 {
                return Err("Windows SendInput Ctrl+V 失败".to_string());
            }
        }

        if auto_send {
            thread::sleep(Duration::from_millis(90));
            let ret_inputs = [key_input(VK_RETURN, false), key_input(VK_RETURN, true)];
            unsafe {
                SendInput(
                    ret_inputs.len() as u32,
                    ret_inputs.as_ptr(),
                    std::mem::size_of::<INPUT>() as i32,
                );
            }
        }
        Ok(())
    }
}

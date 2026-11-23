use image::{DynamicImage, ImageFormat, RgbaImage};
use std::io::Cursor;
use std::path::{Path, PathBuf};

pub enum ClipboardImageInput {
    FilePath(PathBuf),
    RawBytes { bytes: Vec<u8>, alias: String },
}

/// Write an image file path to the OS clipboard as a file reference
/// (macOS: NSURL + NSFilenamesPboardType; Windows: CF_HDROP)
/// so QQ NT receives the file directly and preserves GIF/WebP animation.
pub fn copy_file_to_clipboard(
    path: &Path,
    #[cfg(target_os = "windows")] owner: windows_sys::Win32::Foundation::HWND,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let abs = std::path::absolute(path).map_err(|err| err.to_string())?;
    #[cfg(not(target_os = "windows"))]
    let abs = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf());
    if !abs.exists() {
        return Err(format!("文件不存在: {}", abs.display()));
    }

    #[cfg(target_os = "macos")]
    {
        macos::copy_file_to_pasteboard(&abs)
    }

    #[cfg(target_os = "windows")]
    {
        windows::copy_file_to_clipboard_hdrop(&abs, owner)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = abs;
        Err("当前操作系统暂不支持文件剪贴板写入".to_string())
    }
}

/// Read images or image file paths from the OS clipboard for quick import.
pub fn read_clipboard_images() -> Result<Vec<ClipboardImageInput>, String> {
    // 1. Prefer file references first so animated GIF/WebP files preserve animation
    #[cfg(target_os = "macos")]
    {
        if let Ok(paths) = macos::read_pasteboard_file_paths() {
            if !paths.is_empty() {
                return Ok(paths
                    .into_iter()
                    .map(ClipboardImageInput::FilePath)
                    .collect());
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(paths) = windows::read_clipboard_hdrop_paths() {
            if !paths.is_empty() {
                return Ok(paths
                    .into_iter()
                    .map(ClipboardImageInput::FilePath)
                    .collect());
            }
        }
    }

    // 2. Fallback to raw image bitmap on clipboard via arboard
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("无法访问剪贴板: {e}"))?;
    if let Ok(img_data) = cb.get_image() {
        let rgba = RgbaImage::from_raw(
            img_data.width as u32,
            img_data.height as u32,
            img_data.bytes.into_owned(),
        )
        .ok_or_else(|| "解析剪贴板图片像素失败".to_string())?;
        let mut png_bytes = Vec::new();
        DynamicImage::ImageRgba8(rgba)
            .write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png)
            .map_err(|e| format!("编码剪贴板图片失败: {e}"))?;

        return Ok(vec![ClipboardImageInput::RawBytes {
            bytes: png_bytes,
            alias: "剪贴板图片".to_string(),
        }]);
    }

    Ok(Vec::new())
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send, msg_send_id};
    use objc2_app_kit::NSPasteboard;
    use objc2_foundation::{NSString, NSURL};
    use std::path::{Path, PathBuf};

    pub fn copy_file_to_pasteboard(path: &Path) -> Result<(), String> {
        let path_str = path
            .to_str()
            .ok_or_else(|| "文件路径包含无效 UTF-8".to_string())?;

        unsafe {
            let pb = NSPasteboard::generalPasteboard();
            pb.clearContents();

            let ns_path = NSString::from_str(path_str);
            let ns_url = NSURL::fileURLWithPath(&ns_path);

            let objs: Retained<AnyObject> =
                msg_send_id![class!(NSArray), arrayWithObject: &*ns_url];
            let wrote: bool = msg_send![&pb, writeObjects: &*objs];

            // Also populate legacy NSFilenamesPboardType for maximum compatibility with QQ NT / Electron
            let filenames_type = NSString::from_str("NSFilenamesPboardType");
            let paths_arr: Retained<AnyObject> =
                msg_send_id![class!(NSArray), arrayWithObject: &*ns_path];
            let _: bool = msg_send![&pb, setPropertyList: &*paths_arr, forType: &*filenames_type];

            if !wrote {
                return Err("写入 macOS 剪贴板失败".to_string());
            }
        }
        Ok(())
    }

    pub fn read_pasteboard_file_paths() -> Result<Vec<PathBuf>, String> {
        let mut out = Vec::new();
        unsafe {
            let pb = NSPasteboard::generalPasteboard();
            let filenames_type = NSString::from_str("NSFilenamesPboardType");
            let plist: Option<Retained<AnyObject>> =
                msg_send_id![&pb, propertyListForType: &*filenames_type];
            if let Some(arr_obj) = plist {
                let count: usize = msg_send![&arr_obj, count];
                for i in 0..count {
                    let item: Option<Retained<NSString>> =
                        msg_send_id![&arr_obj, objectAtIndex: i];
                    if let Some(s) = item {
                        let p = PathBuf::from(s.to_string());
                        if p.is_file() {
                            out.push(p);
                        }
                    }
                }
            }
        }
        Ok(out)
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use windows_sys::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND, POINT};
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE, GMEM_ZEROINIT,
    };
    use windows_sys::Win32::UI::Shell::{DragQueryFileW, DROPFILES, HDROP};

    const CF_HDROP: u32 = 15;

    pub fn copy_file_to_clipboard_hdrop(path: &Path, owner: HWND) -> Result<(), String> {
        let wide: Vec<u16> = OsStr::new(path)
            .encode_wide()
            .chain(std::iter::once(0))
            .chain(std::iter::once(0))
            .collect();

        let dropfiles_size = std::mem::size_of::<DROPFILES>();
        let total_bytes = dropfiles_size + wide.len() * std::mem::size_of::<u16>();

        unsafe {
            // EmptyClipboard needs an owner before SetClipboardData can succeed.
            if OpenClipboard(owner) == 0 {
                return Err("打开 Windows 剪贴板失败".to_string());
            }

            if EmptyClipboard() == 0 {
                CloseClipboard();
                return Err("清空 Windows 剪贴板失败".to_string());
            }

            let hglobal: HGLOBAL = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, total_bytes);
            if hglobal.is_null() {
                CloseClipboard();
                return Err("分配剪贴板内存失败".to_string());
            }

            let ptr = GlobalLock(hglobal) as *mut u8;
            if ptr.is_null() {
                GlobalFree(hglobal);
                CloseClipboard();
                return Err("锁定剪贴板内存失败".to_string());
            }

            let df = ptr as *mut DROPFILES;
            (*df).pFiles = dropfiles_size as u32;
            (*df).pt = POINT { x: 0, y: 0 };
            (*df).fNC = 0;
            (*df).fWide = 1;

            let dst = ptr.add(dropfiles_size) as *mut u16;
            std::ptr::copy_nonoverlapping(wide.as_ptr(), dst, wide.len());
            GlobalUnlock(hglobal);

            let res: HANDLE = SetClipboardData(CF_HDROP, hglobal);
            CloseClipboard();

            if res.is_null() {
                GlobalFree(hglobal);
                return Err("写入 CF_HDROP 剪贴板失败".to_string());
            }
        }
        Ok(())
    }

    pub fn read_clipboard_hdrop_paths() -> Result<Vec<PathBuf>, String> {
        let mut paths = Vec::new();
        unsafe {
            if OpenClipboard(std::ptr::null_mut()) == 0 {
                return Ok(paths);
            }
            let handle = GetClipboardData(CF_HDROP);
            if !handle.is_null() {
                let hdrop = handle as HDROP;
                let count = DragQueryFileW(hdrop, 0xFFFF_FFFF, std::ptr::null_mut(), 0);
                for i in 0..count {
                    let len = DragQueryFileW(hdrop, i, std::ptr::null_mut(), 0);
                    if len > 0 {
                        let mut buf = vec![0u16; (len + 1) as usize];
                        DragQueryFileW(hdrop, i, buf.as_mut_ptr(), len + 1);
                        let s = String::from_utf16_lossy(&buf[..len as usize]);
                        let p = PathBuf::from(s);
                        if p.is_file() {
                            paths.push(p);
                        }
                    }
                }
            }
            CloseClipboard();
        }
        Ok(paths)
    }
}

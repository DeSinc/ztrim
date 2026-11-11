#[cfg(windows)]
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg(windows)]
const VIDEO_EXTENSIONS: &[&str] = &[
    ".mp4", ".mkv", ".mov", ".avi", ".webm", ".wmv", ".flv", ".m4v", ".ts", ".m2ts",
];

#[cfg(windows)]
fn reg_add(key: &str, name: Option<&str>, value: &str) -> Result<(), String> {
    let mut cmd = Command::new("reg");
    cmd.arg("add").arg(key);
    if let Some(v_name) = name {
        cmd.arg("/v").arg(v_name);
    } else {
        cmd.arg("/ve");
    }
    cmd.arg("/d").arg(value).arg("/f");
    cmd.creation_flags(CREATE_NO_WINDOW);
    let out = cmd.output().map_err(|e| format!("Failed to run reg add: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    Ok(())
}

#[cfg(windows)]
fn reg_delete(key: &str) {
    let mut cmd = Command::new("reg");
    cmd.args(["delete", key, "/f"]);
    cmd.creation_flags(CREATE_NO_WINDOW);
    let _ = cmd.output();
}

#[cfg(windows)]
pub fn notify_shell_assoc_changed() {
    unsafe {
        windows_sys::Win32::UI::Shell::SHChangeNotify(
            windows_sys::Win32::UI::Shell::SHCNE_ASSOCCHANGED as i32,
            windows_sys::Win32::UI::Shell::SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        );
    }
}

pub fn is_context_menu_registered() -> bool {
    #[cfg(windows)]
    {
        // 1. Check wildcard * shell key
        let mut cmd1 = Command::new("reg");
        cmd1.args(["query", r"HKCU\Software\Classes\*\shell\ztrim"]);
        cmd1.creation_flags(CREATE_NO_WINDOW);
        if let Ok(output) = cmd1.output() {
            if output.status.success() {
                return true;
            }
        }

        // 2. Check generic SystemFileAssociations\video key
        let mut cmd2 = Command::new("reg");
        cmd2.args([
            "query",
            r"HKCU\Software\Classes\SystemFileAssociations\video\shell\ztrim",
        ]);
        cmd2.creation_flags(CREATE_NO_WINDOW);
        if let Ok(output) = cmd2.output() {
            return output.status.success();
        }
    }
    false
}

pub fn register_context_menu() -> Result<(), String> {
    #[cfg(windows)]
    {
        let current_exe = std::env::current_exe()
            .map_err(|e| format!("Could not get current executable path: {e}"))?;
        let exe_path = current_exe.to_string_lossy();
        let cmd_val = format!("\"{}\" \"%1\"", exe_path);
        let icon_val = format!("\"{}\",0", exe_path);
        let menu_label = "Trim & Compress with ztrim";

        // Advanced Query Syntax filter for video files in HKCU\Software\Classes\*\shell
        let applies_to = "System.Kind:=\"video\" OR System.FileExtension:=\".mp4\" OR System.FileExtension:=\".mkv\" OR System.FileExtension:=\".mov\" OR System.FileExtension:=\".avi\" OR System.FileExtension:=\".webm\" OR System.FileExtension:=\".wmv\" OR System.FileExtension:=\".flv\" OR System.FileExtension:=\".m4v\" OR System.FileExtension:=\".ts\" OR System.FileExtension:=\".m2ts\"";

        // 1. Wildcard entry with AppliesTo (standard for modern & classic Windows 10/11 context menus)
        let star_key = r"HKCU\Software\Classes\*\shell\ztrim";
        reg_add(star_key, None, menu_label)?;
        let _ = reg_add(star_key, Some("Icon"), &icon_val);
        let _ = reg_add(star_key, Some("AppliesTo"), applies_to);
        reg_add(&format!(r"{}\command", star_key), None, &cmd_val)?;

        // 2. Generic video type under SystemFileAssociations
        let video_key = r"HKCU\Software\Classes\SystemFileAssociations\video\shell\ztrim";
        let _ = reg_add(video_key, None, menu_label);
        let _ = reg_add(video_key, Some("Icon"), &icon_val);
        let _ = reg_add(&format!(r"{}\command", video_key), None, &cmd_val);

        // 3. Specific video extensions under SystemFileAssociations
        // This ensures the classic context menu displays the verb even if a custom media player
        // (MPC-BE, VLC, etc.) has UserChoice precedence without declaring PerceivedType on its ProgId.
        for ext in VIDEO_EXTENSIONS {
            let ext_key = format!(r"HKCU\Software\Classes\SystemFileAssociations\{}\shell\ztrim", ext);
            let _ = reg_add(&ext_key, None, menu_label);
            let _ = reg_add(&ext_key, Some("Icon"), &icon_val);
            let _ = reg_add(&format!(r"{}\command", ext_key), None, &cmd_val);
        }

        // Notify Explorer shell that associations changed
        notify_shell_assoc_changed();

        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("Context menu registration is only supported on Windows".into())
    }
}

pub fn unregister_context_menu() -> Result<(), String> {
    #[cfg(windows)]
    {
        // 1. Delete wildcard * shell key
        reg_delete(r"HKCU\Software\Classes\*\shell\ztrim");

        // 2. Delete generic video key
        reg_delete(r"HKCU\Software\Classes\SystemFileAssociations\video\shell\ztrim");

        // 3. Delete extension-specific keys
        for ext in VIDEO_EXTENSIONS {
            reg_delete(&format!(r"HKCU\Software\Classes\SystemFileAssociations\{}\shell\ztrim", ext));
        }

        // Notify Explorer shell that associations changed
        notify_shell_assoc_changed();

        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("Context menu registration is only supported on Windows".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extensions_list_not_empty() {
        assert!(VIDEO_EXTENSIONS.contains(&".mp4"));
        assert!(VIDEO_EXTENSIONS.contains(&".mkv"));
        assert!(VIDEO_EXTENSIONS.contains(&".mov"));
        assert!(VIDEO_EXTENSIONS.contains(&".avi"));
        assert!(VIDEO_EXTENSIONS.contains(&".webm"));
    }

    #[test]
    #[cfg(windows)]
    fn test_context_menu_register_unregister_flow() {
        let res = register_context_menu();
        assert!(res.is_ok(), "Registration should succeed: {:?}", res);
        assert!(is_context_menu_registered(), "Should report registered after registering");

        let unreg_res = unregister_context_menu();
        assert!(unreg_res.is_ok(), "Unregistration should succeed: {:?}", unreg_res);
        assert!(!is_context_menu_registered(), "Should report not registered after unregistering");

        // Re-register so user has it ready
        let _ = register_context_menu();
    }
}

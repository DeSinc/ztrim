use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn is_context_menu_registered() -> bool {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("reg");
        cmd.args([
            "query",
            r"HKCU\Software\Classes\SystemFileAssociations\video\shell\ztrim",
        ]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        if let Ok(output) = cmd.output() {
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

        // 1. HKCU\Software\Classes\SystemFileAssociations\video\shell\ztrim
        let key_root = r"HKCU\Software\Classes\SystemFileAssociations\video\shell\ztrim";
        let mut cmd1 = Command::new("reg");
        cmd1.args(["add", key_root, "/ve", "/d", "Trim & Compress with ztrim", "/f"]);
        cmd1.creation_flags(CREATE_NO_WINDOW);
        let out1 = cmd1.output().map_err(|e| format!("Failed to run reg: {e}"))?;
        if !out1.status.success() {
            return Err(String::from_utf8_lossy(&out1.stderr).into_owned());
        }

        // Icon
        let mut cmd_icon = Command::new("reg");
        cmd_icon.args(["add", key_root, "/v", "Icon", "/d", &icon_val, "/f"]);
        cmd_icon.creation_flags(CREATE_NO_WINDOW);
        let _ = cmd_icon.output();

        // Command
        let key_cmd = format!(r"{}\command", key_root);
        let mut cmd2 = Command::new("reg");
        cmd2.args(["add", &key_cmd, "/ve", "/d", &cmd_val, "/f"]);
        cmd2.creation_flags(CREATE_NO_WINDOW);
        let out2 = cmd2.output().map_err(|e| format!("Failed to run reg: {e}"))?;
        if !out2.status.success() {
            return Err(String::from_utf8_lossy(&out2.stderr).into_owned());
        }

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
        let key_root = r"HKCU\Software\Classes\SystemFileAssociations\video\shell\ztrim";
        let mut cmd = Command::new("reg");
        cmd.args(["delete", key_root, "/f"]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        let out = cmd.output().map_err(|e| format!("Failed to run reg: {e}"))?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).into_owned());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("Context menu registration is only supported on Windows".into())
    }
}

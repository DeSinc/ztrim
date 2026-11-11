use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Moves a file to the Windows Recycle Bin using the Win32 Shell API.
pub fn send_to_recycle_bin(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("File does not exist: {}", path.display()));
    }

    #[cfg(windows)]
    {

        // Ensure path uses standard Windows backslashes and is absolute
        let abs_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let path_str = abs_path.to_string_lossy();
        // Strip verbatim prefix \\?\ if present for SHFileOperation compatibility
        let clean_str = if let Some(stripped) = path_str.strip_prefix(r"\\?\") {
            stripped.to_string()
        } else {
            path_str.into_owned()
        };

        let mut wide: Vec<u16> = clean_str.encode_utf16().collect();
        wide.push(0);
        wide.push(0); // double-null terminated

        #[repr(C)]
        struct SHFILEOPSTRUCTW {
            hwnd: *mut std::ffi::c_void,
            w_func: u32,
            p_from: *const u16,
            p_to: *const u16,
            f_flags: u16,
            f_any_operations_aborted: i32,
            h_name_mappings: *mut std::ffi::c_void,
            lpsz_progress_title: *const u16,
        }

        #[link(name = "shell32")]
        unsafe extern "system" {
            fn SHFileOperationW(lpFileOp: *mut SHFILEOPSTRUCTW) -> i32;
        }

        const FO_DELETE: u32 = 0x0003;
        const FOF_ALLOWUNDO: u16 = 0x0040;
        const FOF_NOCONFIRMATION: u16 = 0x0010;
        const FOF_SILENT: u16 = 0x0004;
        const FOF_NOERRORUI: u16 = 0x0400;

        let mut op = SHFILEOPSTRUCTW {
            hwnd: std::ptr::null_mut(),
            w_func: FO_DELETE,
            p_from: wide.as_ptr(),
            p_to: std::ptr::null(),
            f_flags: FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI,
            f_any_operations_aborted: 0,
            h_name_mappings: std::ptr::null_mut(),
            lpsz_progress_title: std::ptr::null(),
        };

        let ret = unsafe { SHFileOperationW(&mut op) };
        if ret == 0 {
            if op.f_any_operations_aborted != 0 {
                Err("Operation was aborted".into())
            } else {
                Ok(())
            }
        } else {
            Err(format!("Failed to send to Recycle Bin (error code: {ret})"))
        }
    }

    #[cfg(not(windows))]
    {
        let _ = path;
        Err("Recycle bin is only supported on Windows".into())
    }
}

/// Restores a file from the Windows Recycle Bin by its filename and original directory.
pub fn restore_from_recycle_bin(file_name: &str, orig_dir: &Path) -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        let clean_dir = orig_dir.canonicalize().unwrap_or_else(|_| orig_dir.to_path_buf());
        let dir_str = clean_dir.to_string_lossy();
        let target_dir = if let Some(stripped) = dir_str.strip_prefix(r"\\?\") {
            stripped.to_string()
        } else {
            dir_str.into_owned()
        };

        let target_expected_path = orig_dir.join(file_name);

        // Escape single quotes for PowerShell script
        let escaped_fn = file_name.replace('\'', "''");
        let escaped_dir = target_dir.replace('\'', "''");

        let ps_script = format!(
            "$shell = New-Object -ComObject Shell.Application;\
             $bin = $shell.Namespace(10);\
             $fn = '{escaped_fn}';\
             $targetDir = '{escaped_dir}';\
             $found = $null;\
             foreach ($item in $bin.Items()) {{\
                 if ($item.Name -eq $fn) {{\
                     $loc = $bin.GetDetailsOf($item, 1);\
                     if (-not $loc -or $loc -eq $targetDir) {{\
                         $found = $item;\
                         break;\
                     }}\
                 }}\
             }};\
             if ($found) {{\
                 $verb = $found.Verbs() | Where-Object {{ $_.Name -replace '&','' -eq 'Restore' }};\
                 if ($verb) {{\
                     $verb.DoIt();\
                     exit 0;\
                 }}\
             }};\
             exit 1;"
        );

        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-NonInteractive", "-Command", &ps_script]);
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());
        cmd.creation_flags(CREATE_NO_WINDOW);

        let status = cmd.status().map_err(|e| format!("Failed to run PowerShell restore: {e}"))?;
        if !status.success() {
            return Err(format!("File '{file_name}' not found in Recycle Bin"));
        }

        // Wait up to 2.5 seconds for the file to reappear on disk
        for _ in 0..50 {
            if target_expected_path.exists() {
                return Ok(target_expected_path);
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        if target_expected_path.exists() {
            Ok(target_expected_path)
        } else {
            Err(format!(
                "Restore command succeeded but file has not appeared yet at: {}",
                target_expected_path.display()
            ))
        }
    }

    #[cfg(not(windows))]
    {
        let _ = (file_name, orig_dir);
        Err("Recycle bin is only supported on Windows".into())
    }
}

/// Performs a full safe Undo operation:
/// 1. Renames the current in-place trimmed file to `{stem}.ztrim_undo_temp.{ext}`.
/// 2. Restores the backup from the Recycle Bin (`backup_file_name`).
/// 3. Renames the restored backup file back to `original_target`.
/// 4. Confirms `original_target` exists and is non-empty.
/// 5. Deletes the temporary `{stem}.ztrim_undo_temp.{ext}`.
pub fn execute_undo(backup_file_name: &str, original_target: &Path) -> Result<(), String> {
    let parent_dir = original_target.parent().unwrap_or_else(|| Path::new("."));
    let file_stem = original_target
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video");
    let ext = original_target
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("mp4");

    let temp_trimmed_path = parent_dir.join(format!("{}.ztrim.{}.{}", file_stem, std::process::id(), ext));

    // Step 1: If current trimmed file exists at original_target, rename it to temp_trimmed_path.
    let mut current_moved = false;
    if original_target.exists() {
        let max_retries = 25; // 2.5 seconds
        let mut last_err = String::new();
        for _ in 1..=max_retries {
            match std::fs::rename(original_target, &temp_trimmed_path) {
                Ok(_) => {
                    current_moved = true;
                    break;
                }
                Err(e) => {
                    last_err = e.to_string();
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }
        if !current_moved {
            return Err(format!(
                "Cannot rename current video (file locked by player or another app: {last_err}). Please close any media players and try again."
            ));
        }
    }

    // Step 2: Restore backup from Recycle Bin
    let restored_path = match restore_from_recycle_bin(backup_file_name, parent_dir) {
        Ok(p) => p,
        Err(e) => {
            // Rollback Step 1: move temp_trimmed_path back to original_target if we moved it
            if current_moved && temp_trimmed_path.exists() {
                let _ = std::fs::rename(&temp_trimmed_path, original_target);
            }
            return Err(format!("Failed to retrieve file from Recycle Bin: {e}"));
        }
    };

    // Step 3: Rename restored backup file to original_target
    let mut restored_renamed = false;
    let mut last_rename_err = String::new();
    for _ in 1..=25 {
        match std::fs::rename(&restored_path, original_target) {
            Ok(_) => {
                restored_renamed = true;
                break;
            }
            Err(e) => {
                last_rename_err = e.to_string();
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }

    if !restored_renamed {
        // If we couldn't rename restored_path to original_target:
        // Do NOT delete restored_path! Leave it safe!
        return Err(format!(
            "Restored original footage as '{}', but could not rename to original name ({last_rename_err}). Your file is safe!",
            restored_path.display()
        ));
    }

    // Step 4: Verify original_target exists and has size
    if !original_target.exists() || std::fs::metadata(original_target).map(|m| m.len()).unwrap_or(0) == 0 {
        return Err("Restored file verification failed: file is missing or zero bytes.".into());
    }

    // Step 5: Clean up temp_trimmed_path now that original is confirmed restored
    if temp_trimmed_path.exists() {
        let _ = std::fs::remove_file(&temp_trimmed_path);
    }

    Ok(())
}

/// Queries the Windows Recycle Bin for all item names currently present.
#[cfg(windows)]
pub fn query_recycle_bin_file_names() -> HashSet<String> {
    let mut set = HashSet::new();

    let ps_script = "$shell = New-Object -ComObject Shell.Application;\
                     $bin = $shell.Namespace(10);\
                     foreach ($item in $bin.Items()) {\
                         Write-Output $item.Name\
                     }";

    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", ps_script]);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    cmd.creation_flags(CREATE_NO_WINDOW);

    if let Ok(child) = cmd.spawn() {
        if let Some(stdout) = child.stdout {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    set.insert(trimmed.to_lowercase());
                }
            }
        }
    }
    set
}

#[cfg(not(windows))]
pub fn query_recycle_bin_file_names() -> HashSet<String> {
    HashSet::new()
}

/// A background watcher that periodically monitors the Recycle Bin
/// so the UI can do instantaneous, lock-free checks to determine if an original
/// file can still be undone, without blocking the UI thread.
pub struct RecycleBinWatcher {
    cached_names: Arc<Mutex<HashSet<String>>>,
    refresh_tx: Sender<()>,
}

impl RecycleBinWatcher {
    pub fn new() -> Self {
        let cached_names = Arc::new(Mutex::new(HashSet::new()));
        let (tx, rx) = channel::<()>();

        let cached = Arc::clone(&cached_names);
        thread::spawn(move || {
            // Initial scan at launch
            let initial = query_recycle_bin_file_names();
            if let Ok(mut lock) = cached.lock() {
                *lock = initial;
            }

            loop {
                // Wait for an explicit refresh signal or timeout after 2 seconds
                let _ = rx.recv_timeout(Duration::from_secs(2));
                let names = query_recycle_bin_file_names();
                if let Ok(mut lock) = cached.lock() {
                    *lock = names;
                }
            }
        });

        Self {
            cached_names,
            refresh_tx: tx,
        }
    }

    /// Triggers an immediate background refresh of the Recycle Bin inventory.
    pub fn refresh(&self) {
        let _ = self.refresh_tx.send(());
    }

    /// Check if a given file name exists in the Recycle Bin.
    pub fn is_file_in_bin(&self, file_name: &str) -> bool {
        if let Ok(lock) = self.cached_names.lock() {
            lock.contains(&file_name.to_lowercase())
        } else {
            false
        }
    }
}

impl Default for RecycleBinWatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recycle_roundtrip() {
        let test_file = std::env::temp_dir().join(format!("ztrim_test_{}.txt", std::process::id()));
        std::fs::write(&test_file, b"test original bytes 12345").expect("write test file");

        assert!(test_file.exists());

        // Send to recycle bin
        send_to_recycle_bin(&test_file).expect("send to recycle bin");
        assert!(!test_file.exists());

        // Restore from recycle bin
        let file_name = test_file.file_name().unwrap().to_str().unwrap();
        let parent = test_file.parent().unwrap();
        let restored = restore_from_recycle_bin(file_name, parent).expect("restore from recycle bin");
        assert!(restored.exists());

        let contents = std::fs::read(&restored).expect("read restored");
        assert_eq!(contents, b"test original bytes 12345");

        // Cleanup
        let _ = std::fs::remove_file(&restored);
    }

    #[test]
    fn test_execute_undo() {
        let temp_dir = std::env::temp_dir();
        let target_file = temp_dir.join(format!("ztrim_undo_test_{}.txt", std::process::id()));
        let backup_name = format!("ztrim_undo_test_{}.orig_bak_99.txt", std::process::id());
        let backup_path = temp_dir.join(&backup_name);

        // 1. Write original file
        std::fs::write(&backup_path, b"ORIGINAL FOOTAGE 12345").expect("write backup");
        // 2. Send backup to recycle bin
        send_to_recycle_bin(&backup_path).expect("send backup to recycle bin");

        // 3. Write trimmed file at target_file
        std::fs::write(&target_file, b"TRIMMED FOOTAGE 99999").expect("write trimmed");

        // 4. Execute Undo
        execute_undo(&backup_name, &target_file).expect("execute undo");

        // 5. Verify target_file has original contents
        assert!(target_file.exists());
        let contents = std::fs::read(&target_file).expect("read restored target");
        assert_eq!(contents, b"ORIGINAL FOOTAGE 12345");

        // Cleanup
        let _ = std::fs::remove_file(&target_file);
    }

    #[test]
    fn test_recycle_bin_watcher() {
        let watcher = RecycleBinWatcher::new();
        // Check non-existent file returns false without panic
        assert!(!watcher.is_file_in_bin("nonexistent_test_video_12345.mp4"));
        watcher.refresh();
    }
}

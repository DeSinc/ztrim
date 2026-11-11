use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Default, Serialize, Deserialize)]
pub struct TrimHistory {
    trimmed_files: HashSet<String>,
}

impl TrimHistory {
    fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("ztrim").join("history.json"))
    }

    pub fn load() -> Self {
        if let Some(path) = Self::config_path() {
            if let Ok(data) = std::fs::read_to_string(path) {
                if let Ok(hist) = serde_json::from_str::<TrimHistory>(&data) {
                    return hist;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        if let Some(path) = Self::config_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(path, json);
            }
        }
    }

    pub fn is_trimmed(&self, path: &Path) -> bool {
        let key = Self::canonical_key(path);
        self.trimmed_files.contains(&key)
    }

    pub fn mark_trimmed(&mut self, path: &Path) {
        let key = Self::canonical_key(path);
        self.trimmed_files.insert(key);
        self.save();
    }

    pub fn unmark_trimmed(&mut self, path: &Path) {
        let key = Self::canonical_key(path);
        self.trimmed_files.remove(&key);
        self.save();
    }

    pub fn clear(&mut self) {
        self.trimmed_files.clear();
        self.save();
    }

    fn canonical_key(path: &Path) -> String {
        path.canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_mark_and_check() {
        let mut hist = TrimHistory::default();
        let path1 = Path::new("C:/Videos/clip1.mp4");
        let path2 = Path::new("C:/Videos/clip2.mp4");

        assert!(!hist.is_trimmed(path1));
        assert!(!hist.is_trimmed(path2));

        hist.trimmed_files.insert(TrimHistory::canonical_key(path1));
        assert!(hist.is_trimmed(path1));
        assert!(!hist.is_trimmed(path2));

        // Case insensitivity test on Windows
        let path1_caps = Path::new("c:/VIDEOS/CLIP1.mp4");
        assert!(hist.is_trimmed(path1_caps));

        // Unmark test
        hist.unmark_trimmed(path1_caps);
        assert!(!hist.is_trimmed(path1));
        assert!(!hist.is_trimmed(path1_caps));
    }
}

use crate::history::TrimHistory;
use std::path::{Path, PathBuf};

const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "mov", "webm", "avi", "flv", "wmv", "m4v", "ts", "m2ts",
];

pub fn is_video_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            let lower = ext.to_lowercase();
            VIDEO_EXTENSIONS.contains(&lower.as_str())
        })
        .unwrap_or(false)
}

pub struct FolderNavigator {
    pub _dir: PathBuf,
    pub videos: Vec<PathBuf>,
}

impl FolderNavigator {
    pub fn from_file(file_path: &Path) -> Self {
        let dir = file_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        let mut videos = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && is_video_file(&path) {
                    // Ignore ztrim temp files
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.contains(".ztrim_temp.") {
                            continue;
                        }
                    }
                    videos.push(path);
                }
            }
        }

        // Sort videos naturally by filename
        videos.sort_by(|a, b| {
            let a_name = a.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let b_name = b.file_name().and_then(|n| n.to_str()).unwrap_or("");
            alphanumeric_sort::compare_str(a_name, b_name)
        });

        Self { _dir: dir, videos }
    }

    pub fn current_index(&self, current: &Path) -> Option<usize> {
        let cur_canon = current.canonicalize().unwrap_or_else(|_| current.to_path_buf());
        self.videos.iter().position(|p| {
            p.canonicalize().unwrap_or_else(|_| p.clone()) == cur_canon
        })
    }

    pub fn next_video(&self, current: &Path) -> Option<PathBuf> {
        let idx = self.current_index(current)?;
        if idx + 1 < self.videos.len() {
            Some(self.videos[idx + 1].clone())
        } else {
            None
        }
    }

    pub fn prev_video(&self, current: &Path) -> Option<PathBuf> {
        let idx = self.current_index(current)?;
        if idx > 0 {
            Some(self.videos[idx - 1].clone())
        } else {
            None
        }
    }

    /// Finds the next untrimmed video in sequence starting after the current video.
    /// If no untrimmed videos exist after current, wraps around to the beginning of the folder
    /// to catch any skipped ones. Returns None if ALL videos in the folder are trimmed.
    pub fn next_untrimmed(&self, current: &Path, history: &TrimHistory) -> Option<PathBuf> {
        if self.videos.is_empty() {
            return None;
        }

        let cur_idx = self.current_index(current).unwrap_or(0);

        // 1. Check forwards from cur_idx + 1 to end of folder
        for i in (cur_idx + 1)..self.videos.len() {
            let p = &self.videos[i];
            if !history.is_trimmed(p) {
                return Some(p.clone());
            }
        }

        // 2. Wrap check from start of folder up to cur_idx
        for i in 0..cur_idx {
            let p = &self.videos[i];
            if !history.is_trimmed(p) {
                return Some(p.clone());
            }
        }

        None
    }

    pub fn first_untrimmed(&self, history: &TrimHistory) -> Option<PathBuf> {
        self.videos.iter().find(|p| !history.is_trimmed(p)).cloned()
    }

    pub fn untrimmed_count(&self, history: &TrimHistory) -> usize {
        self.videos.iter().filter(|p| !history.is_trimmed(p)).count()
    }

    #[allow(dead_code)]
    pub fn trimmed_count(&self, history: &TrimHistory) -> usize {
        self.videos.iter().filter(|p| history.is_trimmed(p)).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alphanumeric_sorting() {
        let mut list = vec!["clip10.mp4", "clip2.mp4", "clip1.mp4", "clip20.mp4"];
        list.sort_by(|a, b| alphanumeric_sort::compare_str(a, b));
        assert_eq!(list, vec!["clip1.mp4", "clip2.mp4", "clip10.mp4", "clip20.mp4"]);
    }

    #[test]
    fn test_next_untrimmed_advance() {
        let nav = FolderNavigator {
            _dir: PathBuf::from("."),
            videos: vec![
                PathBuf::from("clip1.mp4"),
                PathBuf::from("clip2.mp4"),
                PathBuf::from("clip3.mp4"),
            ],
        };

        let mut history = TrimHistory::default();

        // When nothing trimmed, starting from clip1 gives clip2
        let next = nav.next_untrimmed(Path::new("clip1.mp4"), &history);
        assert_eq!(next, Some(PathBuf::from("clip2.mp4")));

        // If clip2 is trimmed, starting from clip1 gives clip3
        history.mark_trimmed(Path::new("clip2.mp4"));
        let next = nav.next_untrimmed(Path::new("clip1.mp4"), &history);
        assert_eq!(next, Some(PathBuf::from("clip3.mp4")));

        // If clip2 and clip3 are trimmed, wrap around to clip1 if untrimmed
        history.mark_trimmed(Path::new("clip3.mp4"));
        let next = nav.next_untrimmed(Path::new("clip3.mp4"), &history);
        assert_eq!(next, Some(PathBuf::from("clip1.mp4")));

        // When all trimmed, returns None
        history.mark_trimmed(Path::new("clip1.mp4"));
        let next = nav.next_untrimmed(Path::new("clip3.mp4"), &history);
        assert_eq!(next, None);

        // Test first_untrimmed, untrimmed_count, trimmed_count
        let mut hist2 = TrimHistory::default();
        assert_eq!(nav.first_untrimmed(&hist2), Some(PathBuf::from("clip1.mp4")));
        assert_eq!(nav.untrimmed_count(&hist2), 3);
        assert_eq!(nav.trimmed_count(&hist2), 0);

        hist2.mark_trimmed(Path::new("clip1.mp4"));
        assert_eq!(nav.first_untrimmed(&hist2), Some(PathBuf::from("clip2.mp4")));
        assert_eq!(nav.untrimmed_count(&hist2), 2);
        assert_eq!(nav.trimmed_count(&hist2), 1);

        hist2.mark_trimmed(Path::new("clip2.mp4"));
        hist2.mark_trimmed(Path::new("clip3.mp4"));
        assert_eq!(nav.first_untrimmed(&hist2), None);
        assert_eq!(nav.untrimmed_count(&hist2), 0);
        assert_eq!(nav.trimmed_count(&hist2), 3);
    }
}

// Simple alphanumeric natural sorting helper
mod alphanumeric_sort {
    use std::cmp::Ordering;

    pub fn compare_str(a: &str, b: &str) -> Ordering {
        let mut a_chars = a.chars().peekable();
        let mut b_chars = b.chars().peekable();

        loop {
            match (a_chars.peek(), b_chars.peek()) {
                (None, None) => return Ordering::Equal,
                (None, Some(_)) => return Ordering::Less,
                (Some(_), None) => return Ordering::Greater,
                (Some(ca), Some(cb)) if ca.is_ascii_digit() && cb.is_ascii_digit() => {
                    let mut num_a: u64 = 0;
                    while let Some(c) = a_chars.peek() {
                        if let Some(digit) = c.to_digit(10) {
                            num_a = num_a.saturating_mul(10).saturating_add(digit as u64);
                            a_chars.next();
                        } else {
                            break;
                        }
                    }

                    let mut num_b: u64 = 0;
                    while let Some(c) = b_chars.peek() {
                        if let Some(digit) = c.to_digit(10) {
                            num_b = num_b.saturating_mul(10).saturating_add(digit as u64);
                            b_chars.next();
                        } else {
                            break;
                        }
                    }

                    match num_a.cmp(&num_b) {
                        Ordering::Equal => continue,
                        ord => return ord,
                    }
                }
                (Some(ca), Some(cb)) => {
                    let ca_lower = ca.to_ascii_lowercase();
                    let cb_lower = cb.to_ascii_lowercase();
                    match ca_lower.cmp(&cb_lower) {
                        Ordering::Equal => {
                            a_chars.next();
                            b_chars.next();
                        }
                        ord => return ord,
                    }
                }
            }
        }
    }
}

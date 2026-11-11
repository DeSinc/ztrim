use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoCodec {
    NvencHevc,
    NvencH264,
    NvencAv1,
    CpuX264,
    CpuX265,
}

impl VideoCodec {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::NvencHevc => "NVIDIA NVENC HEVC (H.265)",
            Self::NvencH264 => "NVIDIA NVENC H.264",
            Self::NvencAv1 => "NVIDIA NVENC AV1",
            Self::CpuX264 => "CPU x264 (Software)",
            Self::CpuX265 => "CPU x265 (Software)",
        }
    }

    pub fn ffmpeg_codec_name(&self) -> &'static str {
        match self {
            Self::NvencHevc => "hevc_nvenc",
            Self::NvencH264 => "h264_nvenc",
            Self::NvencAv1 => "av1_nvenc",
            Self::CpuX264 => "libx264",
            Self::CpuX265 => "libx265",
        }
    }

    pub fn is_nvenc(&self) -> bool {
        matches!(self, Self::NvencHevc | Self::NvencH264 | Self::NvencAv1)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetResolution {
    Source,
    P1440,
    P1080,
    P4K,
    P720,
}

impl TargetResolution {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Source => "Original (Keep Source)",
            Self::P1440 => "1440p (2560x1440)",
            Self::P1080 => "1080p (1920x1080)",
            Self::P4K => "4K (3840x2160)",
            Self::P720 => "720p (1280x720)",
        }
    }

    pub fn max_height(&self) -> Option<u32> {
        match self {
            Self::Source => None,
            Self::P1440 => Some(1440),
            Self::P1080 => Some(1080),
            Self::P4K => Some(2160),
            Self::P720 => Some(720),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QualityPreset {
    pub id: String,
    pub name: String,
    pub resolution: TargetResolution,
    pub avg_bitrate_mbps: f32,
    pub max_bitrate_mbps: f32,
    pub codec: VideoCodec,
    pub audio_bitrate_kbps: u32,
    pub is_builtin: bool,
}

impl QualityPreset {
    pub fn default_presets() -> Vec<Self> {
        vec![
            Self {
                id: "1440p_default".to_string(),
                name: "1440p (9 Mb/s avg, 30 Mb/s max)".to_string(),
                resolution: TargetResolution::P1440,
                avg_bitrate_mbps: 9.0,
                max_bitrate_mbps: 30.0,
                codec: VideoCodec::NvencHevc,
                audio_bitrate_kbps: 192,
                is_builtin: true,
            },
            Self {
                id: "1080p_default".to_string(),
                name: "1080p (6 Mb/s avg, 25 Mb/s max)".to_string(),
                resolution: TargetResolution::P1080,
                avg_bitrate_mbps: 6.0,
                max_bitrate_mbps: 25.0,
                codec: VideoCodec::NvencHevc,
                audio_bitrate_kbps: 192,
                is_builtin: true,
            },
            Self {
                id: "4k_default".to_string(),
                name: "4K (14 Mb/s avg, 45 Mb/s max)".to_string(),
                resolution: TargetResolution::P4K,
                avg_bitrate_mbps: 14.0,
                max_bitrate_mbps: 45.0,
                codec: VideoCodec::NvencHevc,
                audio_bitrate_kbps: 256,
                is_builtin: true,
            },
            Self {
                id: "source_vbr".to_string(),
                name: "Source Res (Custom VBR)".to_string(),
                resolution: TargetResolution::Source,
                avg_bitrate_mbps: 8.0,
                max_bitrate_mbps: 24.0,
                codec: VideoCodec::NvencHevc,
                audio_bitrate_kbps: 192,
                is_builtin: true,
            },
        ]
    }

    fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("ztrim").join("presets.json"))
    }

    pub fn load_presets() -> Vec<Self> {
        let mut list = Self::default_presets();
        if let Some(path) = Self::config_path() {
            if let Ok(data) = std::fs::read_to_string(path) {
                if let Ok(customs) = serde_json::from_str::<Vec<QualityPreset>>(&data) {
                    for c in customs {
                        if !list.iter().any(|p| p.id == c.id) {
                            list.push(c);
                        }
                    }
                }
            }
        }
        list
    }

    pub fn save_presets(presets: &[Self]) {
        if let Some(path) = Self::config_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let customs: Vec<&QualityPreset> = presets.iter().filter(|p| !p.is_builtin).collect();
            if let Ok(json) = serde_json::to_string_pretty(&customs) {
                let _ = std::fs::write(path, json);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_presets() {
        let presets = QualityPreset::default_presets();
        assert_eq!(presets.len(), 4);

        // 1440p preset verification
        let p1440 = presets.iter().find(|p| p.resolution == TargetResolution::P1440).unwrap();
        assert_eq!(p1440.avg_bitrate_mbps, 9.0);
        assert_eq!(p1440.max_bitrate_mbps, 30.0);
        assert_eq!(p1440.codec, VideoCodec::NvencHevc);

        // 1080p preset verification
        let p1080 = presets.iter().find(|p| p.resolution == TargetResolution::P1080).unwrap();
        assert_eq!(p1080.avg_bitrate_mbps, 6.0);
        assert_eq!(p1080.max_bitrate_mbps, 25.0);
        assert_eq!(p1080.codec, VideoCodec::NvencHevc);

        // 4K preset verification
        let p4k = presets.iter().find(|p| p.resolution == TargetResolution::P4K).unwrap();
        assert_eq!(p4k.avg_bitrate_mbps, 14.0);
        assert_eq!(p4k.max_bitrate_mbps, 45.0);
        assert_eq!(p4k.codec, VideoCodec::NvencHevc);
    }
}

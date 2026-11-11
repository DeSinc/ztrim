use crate::context_menu::{is_context_menu_registered, register_context_menu, unregister_context_menu};
use crate::folder::{is_video_file, FolderNavigator};
use crate::history::TrimHistory;
use crate::presets::{AutoQualityLevel, QualityPreset, TargetResolution, VideoCodec};
use crate::render::{JobState, RenderJobRequest, RenderManager};
use crate::video::VideoPlayer;
use eframe::egui::{
    self, vec2, Color32, Margin, Pos2, Rect, RichText, Sense, Stroke, StrokeKind,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn default_true() -> bool {
    true
}

fn default_audio_ratio() -> f32 {
    6.0
}

fn default_audio_thresh() -> f32 {
    -25.0
}

fn default_audio_boost() -> f32 {
    10.0
}

fn default_source_res() -> TargetResolution {
    TargetResolution::Source
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AppConfig {
    pub compress_enabled: bool,
    pub replace_in_place: bool,
    pub selected_preset_idx: usize,
    #[serde(default)]
    pub last_file: Option<PathBuf>,
    #[serde(default)]
    pub has_seen_trim_hint: bool,
    #[serde(default)]
    pub window_width: Option<f32>,
    #[serde(default)]
    pub window_height: Option<f32>,
    #[serde(default)]
    pub window_maximized: bool,
    #[serde(default = "default_true")]
    pub persist_history: bool,
    #[serde(default)]
    pub normalise_audio: bool,
    #[serde(default)]
    pub normalise_audio_apply_to_export: bool,
    #[serde(default = "default_audio_ratio")]
    pub audio_normalise_ratio: f32,
    #[serde(default = "default_audio_thresh")]
    pub audio_normalise_thresh_db: f32,
    #[serde(default = "default_audio_boost")]
    pub audio_normalise_boost_db: f32,
    #[serde(default)]
    pub audio_normalise_advanced_open: bool,
    #[serde(default = "default_source_res")]
    pub selected_res: TargetResolution,
    #[serde(default)]
    pub auto_quality_level: AutoQualityLevel,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            compress_enabled: true,
            replace_in_place: true,
            selected_preset_idx: 0,
            last_file: None,
            has_seen_trim_hint: false,
            window_width: None,
            window_height: None,
            window_maximized: false,
            persist_history: true,
            normalise_audio: false,
            normalise_audio_apply_to_export: false,
            audio_normalise_ratio: 6.0,
            audio_normalise_thresh_db: -25.0,
            audio_normalise_boost_db: 10.0,
            audio_normalise_advanced_open: false,
            selected_res: TargetResolution::Source,
            auto_quality_level: AutoQualityLevel::Med,
        }
    }
}

impl AppConfig {
    fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("ztrim").join("config.json"))
    }

    pub fn load() -> Self {
        if let Some(path) = Self::config_path() {
            if let Ok(data) = std::fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str::<AppConfig>(&data) {
                    return cfg;
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
}

const BG: Color32 = Color32::from_rgb(14, 14, 16);
const PANEL: Color32 = Color32::from_rgb(22, 22, 24);
const CARD: Color32 = Color32::from_rgb(30, 30, 33);
const CARD_HI: Color32 = Color32::from_rgb(42, 42, 46);
const LINE: Color32 = Color32::from_rgb(56, 56, 62);
const ACCENT: Color32 = Color32::from_rgb(37, 99, 235);
const ACCENT_GLOW: Color32 = Color32::from_rgb(96, 165, 250);
const TEXT: Color32 = Color32::from_rgb(238, 240, 245);
const DIM: Color32 = Color32::from_rgb(150, 153, 163);
const GREEN: Color32 = Color32::from_rgb(52, 211, 153);
const GREEN_HOVER: Color32 = Color32::from_rgb(16, 185, 129);
const AMBER: Color32 = Color32::from_rgb(251, 191, 36);
const RED: Color32 = Color32::from_rgb(248, 113, 113);
const BLUE: Color32 = Color32::from_rgb(59, 130, 246);
const UNDO_BANNER_TOTAL_DURATION_SECS: f32 = 10.0;
const UNDO_BANNER_SWIPE_SECS: f32 = 0.350;
const UNDO_BANNER_HOLD_SECS: f32 = UNDO_BANNER_TOTAL_DURATION_SECS - UNDO_BANNER_SWIPE_SECS;

fn draw_update_arrow_icon(p: &egui::Painter, center: Pos2, color: Color32) {
    let stroke = Stroke::new(1.8, color);
    // Vertical stem:
    p.line_segment([center - vec2(0.0, 5.0), center + vec2(0.0, 2.0)], stroke);
    // Chevron pointing down:
    p.line_segment([center + vec2(-4.0, -1.5), center + vec2(0.0, 2.5)], stroke);
    p.line_segment([center + vec2(4.0, -1.5), center + vec2(0.0, 2.5)], stroke);
    // Horizontal tray bar beneath arrow:
    p.line_segment([center + vec2(-5.0, 6.0), center + vec2(5.0, 6.0)], Stroke::new(1.5, color));
}

fn draw_restart_icon(p: &egui::Painter, center: Pos2, color: Color32) {
    let stroke = Stroke::new(1.6, color);
    let r = 5.0;
    let n_pts = 12;
    let start_angle = 0.8f32;
    let end_angle = 5.6f32;
    let pts: Vec<Pos2> = (0..=n_pts)
        .map(|i| {
            let t = i as f32 / n_pts as f32;
            let a = start_angle + t * (end_angle - start_angle);
            center + vec2(r * a.cos(), r * a.sin())
        })
        .collect();
    for w in pts.windows(2) {
        p.line_segment([w[0], w[1]], stroke);
    }
    if let Some(&tip) = pts.last() {
        p.line_segment([tip, tip + vec2(2.5, -2.5)], stroke);
        p.line_segment([tip, tip + vec2(-2.5, -1.0)], stroke);
    }
}

pub fn draw_scissors_icon(p: &egui::Painter, center: Pos2, color: Color32) {
    let stroke = Stroke::new(1.6, color);
    // Two circular finger loops
    p.circle_stroke(center + vec2(-5.0, -3.0), 2.5, stroke);
    p.circle_stroke(center + vec2(-5.0, 3.0), 2.5, stroke);
    // Blades crossing through pivot
    p.line_segment([center + vec2(-2.5, -2.5), center + vec2(6.5, 3.5)], stroke);
    p.line_segment([center + vec2(-2.5, 2.5), center + vec2(6.5, -3.5)], stroke);
    // Pivot pin
    p.circle_filled(center + vec2(0.5, 0.0), 1.2, color);
}

pub fn draw_trash_icon(p: &egui::Painter, center: Pos2, color: Color32) {
    let stroke = Stroke::new(1.4, color);
    // Lid handle loop
    p.line_segment([center + vec2(-2.5, -6.5), center + vec2(2.5, -6.5)], stroke);
    p.line_segment([center + vec2(-2.5, -6.5), center + vec2(-2.5, -5.0)], stroke);
    p.line_segment([center + vec2(2.5, -6.5), center + vec2(2.5, -5.0)], stroke);
    // Lid horizontal bar
    p.line_segment([center + vec2(-6.5, -4.5), center + vec2(6.5, -4.5)], Stroke::new(1.5, color));
    // Bin body (tapered trapezoid)
    p.line_segment([center + vec2(-5.0, -3.5), center + vec2(-4.0, 6.0)], stroke);
    p.line_segment([center + vec2(5.0, -3.5), center + vec2(4.0, 6.0)], stroke);
    p.line_segment([center + vec2(-4.0, 6.0), center + vec2(4.0, 6.0)], stroke);
    // Vertical ribs inside bin
    p.line_segment([center + vec2(0.0, -2.0), center + vec2(0.0, 4.5)], Stroke::new(1.2, color));
    p.line_segment([center + vec2(-2.2, -2.0), center + vec2(-1.8, 4.5)], Stroke::new(1.2, color));
    p.line_segment([center + vec2(2.2, -2.0), center + vec2(1.8, 4.5)], Stroke::new(1.2, color));
}

pub fn setup_custom_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // 1. Embed Outfit (OFL modern smooth rounded geometric font)
    fonts.font_data.insert(
        "outfit".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/Outfit.ttf"))),
    );

    // 2. On Windows, optionally load Microsoft PhagsPa Bold if installed
    #[cfg(windows)]
    {
        if let Ok(phagspa_bytes) = std::fs::read("C:\\Windows\\Fonts\\phagspab.ttf") {
            fonts.font_data.insert(
                "phagspab".to_owned(),
                std::sync::Arc::new(egui::FontData::from_owned(phagspa_bytes)),
            );
        }
    }

    let button_family = egui::FontFamily::Name("button_font".into());
    let family_fonts = fonts.families.entry(button_family).or_default();
    family_fonts.push("outfit".to_owned());
    #[cfg(windows)]
    family_fonts.push("phagspab".to_owned());
    family_fonts.push("Ubuntu-Light".to_owned());

    ctx.set_fonts(fonts);
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TimelineDrag {
    None,
    Playhead,
    TrimStart,
    TrimEnd,
}

pub const END_TRIM_PAD_PX: f32 = 25.0;
pub const START_TRIM_PAD_PX: f32 = 26.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TrimClickDecision {
    SetStartTrim(f64),
    SetEndTrim(f64),
    BlockRightClickFlashRed,
}

pub fn evaluate_trim_click(
    is_right_click: bool,
    pos_x: f32,
    trim_x_min: f32,
    trim_x_max: f32,
    target_sec: f64,
    cur_start: f64,
    cur_end: f64,
    dur: f64,
) -> TrimClickDecision {
    if is_right_click {
        let is_after_end_marker = pos_x >= trim_x_max;
        let min_trim_gap_sec = 0.05;
        let is_inside_or_before_start = !is_after_end_marker
            && (pos_x <= trim_x_min + START_TRIM_PAD_PX || target_sec <= cur_start + min_trim_gap_sec);

        if is_inside_or_before_start {
            TrimClickDecision::BlockRightClickFlashRed
        } else {
            let new_e = target_sec.clamp(cur_start + min_trim_gap_sec, dur);
            TrimClickDecision::SetEndTrim(new_e)
        }
    } else {
        let d_start = (pos_x - trim_x_min).abs();
        let d_end = (pos_x - trim_x_max).abs();

        let is_after_end = pos_x >= trim_x_max;
        let is_in_end_padding = pos_x >= trim_x_max - END_TRIM_PAD_PX;
        let start_overrides = d_start <= START_TRIM_PAD_PX && d_start < d_end;
        let controls_end_trim = is_after_end || (is_in_end_padding && !start_overrides);

        if controls_end_trim {
            let new_e = target_sec.clamp(cur_start + 0.05, dur);
            TrimClickDecision::SetEndTrim(new_e)
        } else {
            let new_s = target_sec.clamp(0.0, (cur_end - 0.05).max(0.0));
            TrimClickDecision::SetStartTrim(new_s)
        }
    }
}

pub fn is_end_trim_flashing_red_at(flash_start: Option<Instant>, now: Instant) -> (bool, bool) {
    if let Some(start) = flash_start {
        let elapsed = now.saturating_duration_since(start).as_millis();
        if elapsed < 400 {
            (true, true)
        } else if elapsed < 600 {
            (false, true)
        } else if elapsed < 1000 {
            (true, true)
        } else {
            (false, false)
        }
    } else {
        (false, false)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrimStateSnapshot {
    pub start: Option<f64>,
    pub end: Option<f64>,
}

#[derive(Clone, Debug)]
pub enum UndoAction {
    Trim { job_id: u64 },
    Delete { job_id: u64 },
}

#[derive(Clone, Debug)]
pub struct UndoBannerState {
    pub file_name: String,
    pub action: UndoAction,
    pub trigger_time: Instant,
    pub dismissing_since: Option<Instant>,
}

pub struct ZTrimApp {
    pub player: Option<VideoPlayer>,
    pub folder_nav: Option<FolderNavigator>,
    pub history: TrimHistory,
    pub render_mgr: RenderManager,
    pub presets: Vec<QualityPreset>,
    pub selected_preset_idx: usize,

    // Compression controls
    pub compress_enabled: bool,
    pub avg_bitrate: f32,
    pub max_bitrate: f32,
    pub selected_codec: VideoCodec,
    pub selected_res: TargetResolution,
    pub audio_bitrate_kbps: u32,
    pub replace_in_place: bool,
    pub auto_quality_level: AutoQualityLevel,

    // Audio normalisation controls
    pub normalise_audio: bool,
    pub normalise_audio_apply_to_export: bool,
    pub audio_normalise_ratio: f32,
    pub audio_normalise_thresh_db: f32,
    pub audio_normalise_boost_db: f32,
    pub audio_normalise_advanced_open: bool,

    // UI state
    pub canvas_zoom: f32,
    pub canvas_pan: egui::Vec2,
    pub status_message: Option<(String, Instant)>,
    pub active_timeline_drag: TimelineDrag,
    pub right_click_trim_anchor: Option<f64>,
    pub end_trim_error_flash_start: Option<Instant>,
    pub prompt_open_trimmed: Option<(PathBuf, Vec<PathBuf>)>,
    pub has_seen_trim_hint: bool,
    pub new_preset_modal_open: bool,
    pub new_preset_name: String,

    // Trim undo/redo stack & scrub state
    pub trim_undo_stack: Vec<TrimStateSnapshot>,
    pub trim_redo_stack: Vec<TrimStateSnapshot>,
    pub trim_drag_initial: Option<TrimStateSnapshot>,
    pub was_playing_before_scrub: bool,
    pub timeline_drag_occurred: bool,

    // Undo banner state
    pub undo_banner: Option<UndoBannerState>,
    pub last_banner_job_id: u64,

    // Persistence & history settings
    pub persist_history: bool,
    pub recycle_bin_watcher: crate::recycle::RecycleBinWatcher,

    // Window and persistence state
    pub last_file: Option<PathBuf>,
    pub window_width: Option<f32>,
    pub window_height: Option<f32>,
    pub window_maximized: bool,
    saved_window_width: Option<f32>,
    saved_window_height: Option<f32>,
    saved_window_maximized: bool,
    window_dirty_since: Option<Instant>,
    startup_time: Instant,
    startup_frames: u32,
    should_maximize_on_start: bool,
    has_seen_maximized_in_session: bool,
    pub updater: std::sync::Arc<std::sync::Mutex<crate::updater::UpdaterState>>,
}

impl ZTrimApp {
    pub fn new(initial_file: Option<PathBuf>, egui_ctx: Option<egui::Context>) -> Self {
        let presets = QualityPreset::load_presets();
        let history = TrimHistory::load();
        let render_mgr = RenderManager::new();
        let config = AppConfig::load();

        if config.persist_history {
            let persisted = RenderManager::load_queue_history();
            render_mgr.load_persisted_jobs(persisted);
        }
        let initial_max_job_id = render_mgr
            .active_jobs
            .lock()
            .map(|jobs| jobs.iter().map(|j| j.job_id).max().unwrap_or(0))
            .unwrap_or(0);
        let recycle_bin_watcher = crate::recycle::RecycleBinWatcher::new();
        let updater = std::sync::Arc::new(std::sync::Mutex::new(crate::updater::UpdaterState::default()));
        if let Some(ref ctx) = egui_ctx {
            crate::updater::check_for_updates(updater.clone(), ctx.clone());
        }

        let initial_preset_idx = if config.selected_preset_idx < presets.len() {
            config.selected_preset_idx
        } else {
            0
        };

        let mut app = Self {
            player: None,
            folder_nav: None,
            history,
            render_mgr,
            presets,
            selected_preset_idx: initial_preset_idx,
            compress_enabled: config.compress_enabled,
            avg_bitrate: 9.0,
            max_bitrate: 30.0,
            selected_codec: VideoCodec::NvencHevc,
            selected_res: config.selected_res,
            audio_bitrate_kbps: 192,
            replace_in_place: config.replace_in_place,
            auto_quality_level: config.auto_quality_level,
            normalise_audio: config.normalise_audio,
            normalise_audio_apply_to_export: config.normalise_audio_apply_to_export,
            audio_normalise_ratio: config.audio_normalise_ratio,
            audio_normalise_thresh_db: config.audio_normalise_thresh_db,
            audio_normalise_boost_db: config.audio_normalise_boost_db,
            audio_normalise_advanced_open: config.audio_normalise_advanced_open,
            canvas_zoom: 1.0,
            canvas_pan: vec2(0.0, 0.0),
            status_message: None,
            active_timeline_drag: TimelineDrag::None,
            right_click_trim_anchor: None,
            end_trim_error_flash_start: None,
            prompt_open_trimmed: None,
            has_seen_trim_hint: config.has_seen_trim_hint,
            new_preset_modal_open: false,
            new_preset_name: String::new(),
            trim_undo_stack: Vec::new(),
            trim_redo_stack: Vec::new(),
            trim_drag_initial: None,
            was_playing_before_scrub: false,
            timeline_drag_occurred: false,
            undo_banner: None,
            last_banner_job_id: initial_max_job_id,
            persist_history: config.persist_history,
            recycle_bin_watcher,
            last_file: config.last_file.clone(),
            window_width: config.window_width,
            window_height: config.window_height,
            window_maximized: config.window_maximized,
            saved_window_width: config.window_width,
            saved_window_height: config.window_height,
            saved_window_maximized: config.window_maximized,
            window_dirty_since: None,
            startup_time: Instant::now(),
            startup_frames: 0,
            should_maximize_on_start: config.window_maximized,
            has_seen_maximized_in_session: false,
            updater,
        };

        app.sync_from_preset_idx(initial_preset_idx);

        let file_to_open = initial_file.or_else(|| {
            config.last_file.as_ref().and_then(|p| {
                if p.exists() {
                    Some(p.clone())
                } else {
                    None
                }
            })
        });

        if let Some(path) = file_to_open {
            let autoplay = false;
            app.load_video_file(&path, autoplay);
        }

        app
    }

    pub fn save_config(&mut self) {
        let last_file = self.player.as_ref().map(|p| p.file_path.clone()).or_else(|| self.last_file.clone());
        self.last_file = last_file.clone();
        let cfg = AppConfig {
            compress_enabled: self.compress_enabled,
            replace_in_place: self.replace_in_place,
            selected_preset_idx: self.selected_preset_idx,
            last_file,
            has_seen_trim_hint: self.has_seen_trim_hint,
            window_width: self.window_width,
            window_height: self.window_height,
            window_maximized: self.window_maximized,
            persist_history: self.persist_history,
            normalise_audio: self.normalise_audio,
            normalise_audio_apply_to_export: self.normalise_audio_apply_to_export,
            audio_normalise_ratio: self.audio_normalise_ratio,
            audio_normalise_thresh_db: self.audio_normalise_thresh_db,
            audio_normalise_boost_db: self.audio_normalise_boost_db,
            audio_normalise_advanced_open: self.audio_normalise_advanced_open,
            selected_res: self.selected_res,
            auto_quality_level: self.auto_quality_level,
        };
        cfg.save();
        self.saved_window_width = self.window_width;
        self.saved_window_height = self.window_height;
        self.saved_window_maximized = self.window_maximized;
        self.window_dirty_since = None;
    }

    pub fn is_auto_preset(&self) -> bool {
        self.presets
            .get(self.selected_preset_idx)
            .map_or(false, |p| p.id == "auto")
    }

    pub fn apply_auto_bitrate(&mut self) {
        let orig_h = self.player.as_ref().map(|p| p.original_height).unwrap_or(1080);
        let effective_h = if let Some(max_h) = self.selected_res.max_height() {
            orig_h.min(max_h)
        } else {
            orig_h
        };
        let (avg, max) = crate::presets::auto_bitrate_range(effective_h, self.auto_quality_level);
        self.avg_bitrate = avg;
        self.max_bitrate = max;
    }

    pub fn sync_from_preset_idx(&mut self, idx: usize) {
        if idx < self.presets.len() {
            self.selected_preset_idx = idx;
            let p = &self.presets[idx];
            self.selected_codec = p.codec;
            self.audio_bitrate_kbps = p.audio_bitrate_kbps;
            if p.id == "auto" {
                self.apply_auto_bitrate();
            } else {
                self.avg_bitrate = p.avg_bitrate_mbps;
                self.max_bitrate = p.max_bitrate_mbps;
                self.selected_res = p.resolution;
            }
        }
    }

    pub fn sync_player_audio_normalisation(&mut self) {
        if let Some(ref mut p) = self.player {
            let norm_cfg = if self.normalise_audio {
                Some(crate::video::AudioNormaliseConfig {
                    ratio: self.audio_normalise_ratio,
                    threshold_db: self.audio_normalise_thresh_db,
                    boost_db: self.audio_normalise_boost_db,
                })
            } else {
                None
            };
            p.reload_audio(norm_cfg);
        }
    }

    pub fn load_video_file(&mut self, path: &Path, autoplay: bool) {
        if !path.exists() {
            self.set_status(format!("File does not exist: {}", path.display()));
            return;
        }

        let norm_cfg = if self.normalise_audio {
            Some(crate::video::AudioNormaliseConfig {
                ratio: self.audio_normalise_ratio,
                threshold_db: self.audio_normalise_thresh_db,
                boost_db: self.audio_normalise_boost_db,
            })
        } else {
            None
        };

        match VideoPlayer::load_from_file(path, norm_cfg) {
            Ok(mut player) => {
                let nav = FolderNavigator::from_file(path);
                self.folder_nav = Some(nav);
                if autoplay {
                    player.play();
                }
                self.player = Some(player);
                self.last_file = Some(path.to_path_buf());
                self.canvas_zoom = 1.0;
                self.canvas_pan = vec2(0.0, 0.0);
                self.right_click_trim_anchor = None;
                self.end_trim_error_flash_start = None;
                self.active_timeline_drag = TimelineDrag::None;
                self.trim_undo_stack.clear();
                self.trim_redo_stack.clear();
                self.trim_drag_initial = None;
                self.was_playing_before_scrub = false;
                if self.is_auto_preset() {
                    self.apply_auto_bitrate();
                }
                self.save_config();
                self.set_status(format!("Loaded: {}", path.file_name().unwrap_or_default().to_string_lossy()));
            }
            Err(e) => {
                self.set_status(format!("Failed to load video: {e}"));
            }
        }
    }

    pub fn record_trim_undo(&mut self, snapshot: TrimStateSnapshot) {
        if self.trim_undo_stack.last() == Some(&snapshot) {
            return;
        }
        self.trim_undo_stack.push(snapshot);
        if self.trim_undo_stack.len() > 100 {
            self.trim_undo_stack.remove(0);
        }
        self.trim_redo_stack.clear();
    }

    pub fn undo_trim(&mut self) -> bool {
        let player = match self.player.as_mut() {
            Some(p) => p,
            None => return false,
        };
        if let Some(prev) = self.trim_undo_stack.pop() {
            let current = TrimStateSnapshot {
                start: player.trim_start_sec,
                end: player.trim_end_sec,
            };
            self.trim_redo_stack.push(current);
            player.trim_start_sec = prev.start;
            player.trim_end_sec = prev.end;

            if let Some(s) = prev.start {
                player.seek_to_time(s);
            } else if let Some(e) = prev.end {
                player.seek_to_time(e);
            }

            self.set_status("Undo: Restored trim markers".into());
            true
        } else {
            false
        }
    }

    pub fn redo_trim(&mut self) -> bool {
        let player = match self.player.as_mut() {
            Some(p) => p,
            None => return false,
        };
        if let Some(next) = self.trim_redo_stack.pop() {
            let current = TrimStateSnapshot {
                start: player.trim_start_sec,
                end: player.trim_end_sec,
            };
            self.trim_undo_stack.push(current);
            player.trim_start_sec = next.start;
            player.trim_end_sec = next.end;

            if let Some(s) = next.start {
                player.seek_to_time(s);
            } else if let Some(e) = next.end {
                player.seek_to_time(e);
            }

            self.set_status("Redo: Restored trim markers".into());
            true
        } else {
            false
        }
    }

    pub fn skip_next(&mut self) {
        let mut next_target = None;
        if let (Some(nav), Some(player)) = (&self.folder_nav, &self.player) {
            next_target = nav.next_untrimmed(&player.file_path, &self.history)
                .or_else(|| nav.next_video(&player.file_path));
        }
        if let Some(path) = next_target {
            self.load_video_file(&path, true);
            self.set_status(format!(
                "Skipped to: {}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ));
        } else {
            self.set_status("No further videos in folder".into());
        }
    }

    pub fn go_prev(&mut self) {
        let mut prev_target = None;
        if let (Some(nav), Some(player)) = (&self.folder_nav, &self.player) {
            prev_target = nav.prev_video(&player.file_path);
        }
        if let Some(path) = prev_target {
            self.load_video_file(&path, true);
            self.set_status(format!(
                "Previous: {}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ));
        } else {
            self.set_status("Already at first video in folder".into());
        }
    }

    pub fn set_status(&mut self, msg: String) {
        self.status_message = Some((msg, Instant::now()));
    }

    pub fn submit_current_job_and_advance(&mut self) {
        let player = match self.player.as_ref() {
            Some(p) => p,
            None => return,
        };

        let input_path = player.file_path.clone();
        let trim_start = if player.trim_start() > 0.01 {
            Some(player.trim_start())
        } else {
            None
        };
        let trim_end = if player.trim_end() < (player.duration_sec - 0.01) {
            Some(player.trim_end())
        } else {
            None
        };

        let current_preset = QualityPreset {
            id: "current_render".into(),
            name: "Current Settings".into(),
            resolution: self.selected_res,
            avg_bitrate_mbps: self.avg_bitrate,
            max_bitrate_mbps: self.max_bitrate,
            codec: self.selected_codec,
            audio_bitrate_kbps: self.audio_bitrate_kbps,
            is_builtin: false,
        };

        let req = RenderJobRequest {
            id: 0,
            input_path: input_path.clone(),
            trim_start,
            trim_end,
            total_duration: player.duration_sec,
            compress: self.compress_enabled,
            preset: current_preset,
            replace_in_place: self.replace_in_place,
            normalise_audio: self.normalise_audio && self.normalise_audio_apply_to_export,
            audio_normalise_ratio: self.audio_normalise_ratio,
            audio_normalise_thresh_db: self.audio_normalise_thresh_db,
            audio_normalise_boost_db: self.audio_normalise_boost_db,
        };

        // Enqueue job on background worker thread
        self.render_mgr.submit_job(req);
        if self.persist_history {
            self.render_mgr.save_queue_history();
        }

        // Mark in history log so we don't sequence back over it
        self.history.mark_trimmed(&input_path);

        self.set_status(format!(
            "Queued render for {}",
            input_path.file_name().unwrap_or_default().to_string_lossy()
        ));

        // Advance immediately to the next untrimmed video in folder!
        if let Some(ref nav) = self.folder_nav {
            if let Some(next_path) = nav.next_untrimmed(&input_path, &self.history) {
                if let Some(ref mut p) = self.player {
                    p.stop_all_workers();
                }
                self.load_video_file(&next_path, true);
            } else {
                if let Some(ref mut p) = self.player {
                    p.pause();
                    p.stop_all_workers();
                }
                self.set_status("All videos in folder have been processed!".into());
            }
        }
    }

    pub fn delete_current_clip(&mut self) {
        let player = match self.player.as_mut() {
            Some(p) => p,
            None => return,
        };

        let file_path = player.file_path.clone();
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("video")
            .to_string();

        // Stop workers and close player to release file handle on Windows
        player.stop_all_workers();
        self.player = None;

        // Send file to Windows Recycle Bin
        match crate::recycle::send_to_recycle_bin(&file_path) {
            Ok(_) => {
                let job_id = self.render_mgr.record_deleted_job(file_path.clone());
                if self.persist_history {
                    self.render_mgr.save_queue_history();
                }
                self.recycle_bin_watcher.refresh();
                self.last_banner_job_id = self.last_banner_job_id.max(job_id);
                self.undo_banner = Some(UndoBannerState {
                    file_name: file_name.clone(),
                    action: UndoAction::Delete { job_id },
                    trigger_time: Instant::now(),
                    dismissing_since: None,
                });
                self.set_status(format!("Moved {} to Recycle Bin", file_name));

                // Advance to next video in folder
                if let Some(ref nav) = self.folder_nav {
                    if let Some(next_path) = nav.next_untrimmed(&file_path, &self.history).or_else(|| nav.next_video(&file_path)) {
                        self.load_video_file(&next_path, true);
                    } else {
                        self.set_status("No further videos in folder".into());
                    }
                }
            }
            Err(e) => {
                // If deletion failed, reload the video file
                self.load_video_file(&file_path, false);
                self.set_status(format!("Failed to delete file: {e}"));
            }
        }
    }

    pub fn undo_job(&mut self, job_id: u64) {
        let job_info = {
            if let Ok(jobs) = self.render_mgr.active_jobs.lock() {
                jobs.iter().find(|j| j.job_id == job_id).cloned()
            } else {
                None
            }
        };

        let job = match job_info {
            Some(j) => j,
            None => return,
        };

        if job.is_undone {
            return;
        }

        match &job.state {
            JobState::Finished => {
                let backup_name = match &job.backup_file_name {
                    Some(b) => b.clone(),
                    None => {
                        self.set_status("No backup file found in Recycle Bin".into());
                        return;
                    }
                };

                let is_current = self.player.as_ref().map(|p| p.file_path == job.input_path).unwrap_or(false);
                if is_current {
                    if let Some(ref mut p) = self.player {
                        p.stop_all_workers();
                    }
                    self.player = None;
                }

                match crate::recycle::execute_undo(&backup_name, &job.input_path) {
                    Ok(_) => {
                        self.history.unmark_trimmed(&job.input_path);
                        if let Ok(mut jobs) = self.render_mgr.active_jobs.lock() {
                            if let Some(j) = jobs.iter_mut().find(|j| j.job_id == job_id) {
                                j.is_undone = true;
                                j.undone_at = Some(Instant::now());
                            }
                        }
                        if let Some(ref b) = self.undo_banner {
                            if let UndoAction::Trim { job_id: b_id } = b.action {
                                if b_id == job_id {
                                    self.undo_banner = None;
                                }
                            }
                        }
                        if self.persist_history {
                            self.render_mgr.save_queue_history();
                        }
                        self.recycle_bin_watcher.refresh();
                        self.set_status(format!("Restored original video for {}", job.file_name));
                        // Load restored file back into player to inspect immediately!
                        self.load_video_file(&job.input_path, false);
                    }
                    Err(e) => {
                        if is_current {
                            self.load_video_file(&job.input_path, false);
                        }
                        self.set_status(format!("Undo failed: {e}"));
                    }
                }
            }
            JobState::Deleted => {
                let parent_dir = job.input_path.parent().unwrap_or_else(|| Path::new("."));
                match crate::recycle::restore_from_recycle_bin(&job.file_name, parent_dir) {
                    Ok(_) => {
                        if let Ok(mut jobs) = self.render_mgr.active_jobs.lock() {
                            if let Some(j) = jobs.iter_mut().find(|j| j.job_id == job_id) {
                                j.is_undone = true;
                                j.undone_at = Some(Instant::now());
                            }
                        }
                        if let Some(ref b) = self.undo_banner {
                            if let UndoAction::Delete { job_id: b_id } = b.action {
                                if b_id == job_id {
                                    self.undo_banner = None;
                                }
                            }
                        }
                        if self.persist_history {
                            self.render_mgr.save_queue_history();
                        }
                        self.recycle_bin_watcher.refresh();
                        self.set_status(format!("Restored deleted clip: {}", job.file_name));
                        // Load restored video back into canvas!
                        self.load_video_file(&job.input_path, true);
                    }
                    Err(e) => {
                        self.set_status(format!("Restore failed: {e}"));
                    }
                }
            }
            _ => {}
        }
    }

    pub fn undo_current_banner(&mut self) {
        let action = match self.undo_banner.as_ref() {
            Some(b) => b.action.clone(),
            None => return,
        };

        match action {
            UndoAction::Trim { job_id, .. } => {
                self.undo_job(job_id);
            }
            UndoAction::Delete { job_id, .. } => {
                self.undo_job(job_id);
            }
        }
    }

    fn action_button_text(&self) -> &'static str {
        let has_trim = self.player.as_ref().map_or(false, |p| p.has_custom_trim());
        let normalise_export = self.normalise_audio && self.normalise_audio_apply_to_export;
        if self.compress_enabled {
            if has_trim {
                "TRIM & COMPRESS"
            } else {
                "COMPRESS"
            }
        } else if normalise_export {
            if has_trim {
                "TRIM & NORMALISE"
            } else {
                "NORMALISE AUDIO"
            }
        } else {
            "TRIM (LOSSLESS)"
        }
    }
}

impl eframe::App for ZTrimApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.render_mgr.update() {
            if self.persist_history {
                self.render_mgr.save_queue_history();
            }
            self.recycle_bin_watcher.refresh();
        }

        // Check for completed render jobs to trigger 10-second undo banner
        if let Ok(jobs) = self.render_mgr.active_jobs.lock() {
            for j in jobs.iter() {
                if j.state == JobState::Finished && j.job_id > self.last_banner_job_id {
                    self.last_banner_job_id = j.job_id;
                    if let Some(ref _backup_name) = j.backup_file_name {
                        self.undo_banner = Some(UndoBannerState {
                            file_name: j.file_name.clone(),
                            action: UndoAction::Trim { job_id: j.job_id },
                            trigger_time: Instant::now(),
                            dismissing_since: None,
                        });
                    }
                }
            }
        }

        // Manage undo banner timer and swipe-out animation
        if let Some(ref b) = self.undo_banner {
            let expired = if let Some(dt) = b.dismissing_since {
                dt.elapsed().as_secs_f32() >= UNDO_BANNER_SWIPE_SECS
            } else {
                b.trigger_time.elapsed().as_secs_f32() >= UNDO_BANNER_TOTAL_DURATION_SECS
            };

            let is_animating = if b.dismissing_since.is_some() {
                true
            } else {
                b.trigger_time.elapsed().as_secs_f32() >= UNDO_BANNER_HOLD_SECS
            };

            if expired {
                self.undo_banner = None;
            } else if is_animating {
                ctx.request_repaint();
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }

        self.startup_frames = self.startup_frames.saturating_add(1);

        // Maximize on startup via ViewportCommand if config requested it,
        // avoiding the Win32 CreateWindowEx pinned-size bug when with_inner_size and with_maximized are combined.
        if self.should_maximize_on_start {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
        }

        // Track window dimensions and maximized status
        let (vp_inner_rect, is_maximized, is_minimized, close_requested) = ctx.input(|i| {
            let vp = i.viewport();
            (
                vp.inner_rect,
                vp.maximized.unwrap_or(false),
                vp.minimized.unwrap_or(false),
                vp.close_requested(),
            )
        });

        if is_maximized || self.startup_frames >= 3 {
            self.should_maximize_on_start = false;
        }

        if !is_minimized {
            if is_maximized {
                if !self.window_maximized {
                    self.window_maximized = true;
                    self.has_seen_maximized_in_session = true;
                    self.window_dirty_since = Some(Instant::now());
                } else if !self.has_seen_maximized_in_session {
                    self.has_seen_maximized_in_session = true;
                }
            } else {
                let is_unmaximizing = self.window_maximized
                    && (self.has_seen_maximized_in_session || self.startup_time.elapsed().as_millis() > 1000);
                if is_unmaximizing {
                    self.window_maximized = false;
                    self.window_dirty_since = Some(Instant::now());
                }

                // Only record user resized window dimensions when NOT maximized and NOT minimized
                if !self.window_maximized {
                    if let Some(rect) = vp_inner_rect {
                        let w = rect.width();
                        let h = rect.height();
                        if w >= 920.0 && h >= 580.0 {
                            let size_changed = match (self.window_width, self.window_height) {
                                (Some(sw), Some(sh)) => (sw - w).abs() > 1.0 || (sh - h).abs() > 1.0,
                                _ => true,
                            };
                            if size_changed {
                                self.window_width = Some(w);
                                self.window_height = Some(h);
                                self.window_dirty_since = Some(Instant::now());
                            }
                        }
                    }
                }
            }
        }

        if close_requested {
            self.save_config();
        } else if let Some(dirty_time) = self.window_dirty_since {
            if dirty_time.elapsed().as_millis() >= 1000 {
                self.save_config();
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(1000));
            }
        }

        // Keyboard shortcuts
        let wants_kb = ctx.egui_wants_keyboard_input();
        if !wants_kb {
            // Undo / Redo for trim marker movements:
            // Ctrl+Z (Undo)
            if ctx.input(|i| i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::Z)) {
                self.undo_trim();
            }
            // Ctrl+Shift+Z or Ctrl+Y (Redo)
            if ctx.input(|i| {
                (i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Z))
                    || (i.modifiers.command && i.key_pressed(egui::Key::Y))
            }) {
                self.redo_trim();
            }

            if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                if let Some(ref mut p) = self.player {
                    p.toggle_play();
                }
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                if let Some(ref mut p) = self.player {
                    p.step_frames(-1);
                }
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                if let Some(ref mut p) = self.player {
                    p.step_frames(1);
                }
            }
            if ctx.input(|i| i.key_pressed(egui::Key::I)) {
                if let Some(ref mut p) = self.player {
                    let snapshot = TrimStateSnapshot {
                        start: p.trim_start_sec,
                        end: p.trim_end_sec,
                    };
                    p.set_trim_start_to_current();
                    if p.trim_start_sec != snapshot.start {
                        self.record_trim_undo(snapshot);
                    }
                }
            }
            if ctx.input(|i| i.key_pressed(egui::Key::O)) {
                if let Some(ref mut p) = self.player {
                    let snapshot = TrimStateSnapshot {
                        start: p.trim_start_sec,
                        end: p.trim_end_sec,
                    };
                    p.set_trim_end_to_current();
                    if p.trim_end_sec != snapshot.end {
                        self.record_trim_undo(snapshot);
                    }
                }
            }
            // Skip forward to next untrimmed video with PageDown or S
            if ctx.input(|i| i.key_pressed(egui::Key::PageDown) || i.key_pressed(egui::Key::S)) {
                self.skip_next();
            }
            // Go to previous video with PageUp
            if ctx.input(|i| i.key_pressed(egui::Key::PageUp)) {
                self.go_prev();
            }
            // Ctrl+Enter to trigger COMPRESS/TRIM
            if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter)) {
                self.submit_current_job_and_advance();
            }
            // Delete key to delete clip
            if ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
                self.delete_current_clip();
            }
        }

        // Drag & drop file/folder support
        let dropped_paths: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        for p in dropped_paths {
            if p.is_dir() {
                let nav = FolderNavigator::from_file(&p.join("placeholder.mp4"));
                if nav.videos.is_empty() {
                    self.set_status("No video files found in folder".into());
                } else if let Some(untrimmed) = nav.first_untrimmed(&self.history) {
                    let untrimmed_cnt = nav.untrimmed_count(&self.history);
                    self.set_status(format!("Opened folder: {} untrimmed clips", untrimmed_cnt));
                    self.load_video_file(&untrimmed, true);
                } else {
                    self.prompt_open_trimmed = Some((p, nav.videos.clone()));
                }
                break;
            } else if is_video_file(&p) {
                self.load_video_file(&p, true);
                break;
            }
        }

        // Top bar
        egui::Panel::top("top_panel")
            .frame(egui::Frame::new().fill(PANEL).inner_margin(Margin::symmetric(14, 8)))
            .show(ui, |ui| {
                self.render_top_bar(ui);
            });

        // Right side control panel
        egui::Panel::right("right_panel")
            .default_size(360.0)
            .min_size(320.0)
            .max_size(480.0)
            .resizable(true)
            .frame(egui::Frame::new().fill(PANEL).inner_margin(Margin::symmetric(14, 10)))
            .show(ui, |ui| {
                self.render_right_pane(ui);
            });

        // Main canvas area
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(Margin::symmetric(12, 10)))
            .show(ui, |ui| {
                self.render_canvas_and_timeline(ui, &ctx);
            });

        // Dialog prompt: All Videos Already Trimmed
        if let Some((_dir, ref vids)) = self.prompt_open_trimmed.clone() {
            egui::Window::new("All Videos Already Trimmed")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
                .show(&ctx, |ui| {
                    ui.label(RichText::new("No Untrimmed Videos Found").size(14.0).strong().color(TEXT));
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!(
                            "All {} video{} in this folder have already been trimmed and processed.",
                            vids.len(),
                            if vids.len() == 1 { "" } else { "s" }
                        ))
                        .color(DIM),
                    );
                    ui.add_space(4.0);
                    ui.label("Would you like to open the first trimmed video anyway to review or re-trim?");
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let open_btn = egui::Button::new(RichText::new("Open First Trimmed Video").strong().color(Color32::WHITE))
                            .fill(ACCENT);
                        if ui.add(open_btn).clicked() {
                            if let Some(first) = vids.first() {
                                self.load_video_file(first, true);
                            }
                            self.prompt_open_trimmed = None;
                        }
                        if ui.button("Cancel").clicked() {
                            self.prompt_open_trimmed = None;
                        }
                    });
                });
        }

        // New Preset modal dialog
        if self.new_preset_modal_open {
            egui::Window::new("Save as New Preset")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
                .show(&ctx, |ui| {
                    ui.label("Enter preset name:");
                    ui.text_edit_singleline(&mut self.new_preset_name);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() && !self.new_preset_name.trim().is_empty() {
                            let new_p = QualityPreset {
                                id: format!("custom_{}", Instant::now().elapsed().as_micros()),
                                name: self.new_preset_name.trim().to_string(),
                                resolution: self.selected_res,
                                avg_bitrate_mbps: self.avg_bitrate,
                                max_bitrate_mbps: self.max_bitrate,
                                codec: self.selected_codec,
                                audio_bitrate_kbps: self.audio_bitrate_kbps,
                                is_builtin: false,
                            };
                            self.presets.push(new_p);
                            self.selected_preset_idx = self.presets.len() - 1;
                            QualityPreset::save_presets(&self.presets);
                            self.new_preset_modal_open = false;
                            self.new_preset_name.clear();
                        }
                        if ui.button("Cancel").clicked() {
                            self.new_preset_modal_open = false;
                            self.new_preset_name.clear();
                        }
                    });
                });
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_config();
    }
}

impl ZTrimApp {
    fn render_update_button(&self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let (up_avail, latest, is_dl, dl_prog, applied, checked) = {
            let st = self.updater.lock().unwrap();
            (
                st.update_available,
                st.latest_version.clone(),
                st.downloading,
                st.download_progress,
                st.update_applied,
                st.checked,
            )
        };

        if applied {
            let (rect, resp) = ui.allocate_exact_size(vec2(80.0, 24.0), Sense::click());
            let bg = if resp.hovered() {
                GREEN_HOVER
            } else {
                GREEN
            };
            let p = ui.painter();
            p.rect_filled(rect, 12.0, bg);
            draw_restart_icon(&p, rect.left_center() + vec2(14.0, 0.0), Color32::WHITE);
            p.text(
                rect.center() + vec2(8.0, 0.0),
                egui::Align2::CENTER_CENTER,
                "Restart",
                egui::FontId::proportional(12.0),
                Color32::WHITE,
            );
            let resp = resp.on_hover_text("Update ready! Click to restart ztrim now");
            if resp.clicked() {
                crate::updater::restart_application();
            }
        } else if is_dl {
            let (rect, resp) = ui.allocate_exact_size(vec2(72.0, 24.0), Sense::hover());
            let p = ui.painter();
            p.rect_filled(rect, 12.0, Color32::from_rgb(20, 24, 34));
            p.rect_stroke(rect, 12.0, Stroke::new(1.0, GREEN), StrokeKind::Inside);
            draw_update_arrow_icon(&p, rect.left_center() + vec2(14.0, 0.0), GREEN);
            p.text(
                rect.center() + vec2(8.0, 0.0),
                egui::Align2::CENTER_CENTER,
                format!("{:.0}%", dl_prog * 100.0),
                egui::FontId::proportional(11.5),
                Color32::WHITE,
            );
            let _ = resp.on_hover_text(format!("Downloading update: {:.0}%", dl_prog * 100.0));
        } else if up_avail {
            let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 24.0), Sense::click());
            let bg = if resp.hovered() {
                GREEN_HOVER
            } else {
                GREEN
            };
            let p = ui.painter();
            p.rect_filled(rect, 12.0, bg);
            draw_update_arrow_icon(&p, rect.center(), Color32::WHITE);
            let resp = resp.on_hover_text(format!("Update v{latest} available! Click to update now"));
            if resp.clicked() {
                crate::updater::download_and_install_update(self.updater.clone(), ctx.clone());
            }
        } else {
            let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 24.0), Sense::click());
            let bg = if resp.hovered() { CARD_HI } else { Color32::TRANSPARENT };
            let p = ui.painter();
            if resp.hovered() {
                p.rect_filled(rect, 4.0, bg);
            }
            let icon_col = if resp.hovered() { TEXT } else { DIM };
            draw_update_arrow_icon(&p, rect.center(), icon_col);
            let cur_ver = env!("CARGO_PKG_VERSION");
            let tip = if checked {
                format!("ztrim v{cur_ver} (Up to date)\nClick to check for updates")
            } else {
                format!("ztrim v{cur_ver}\nClick to check for updates")
            };
            let resp = resp.on_hover_text(tip);
            if resp.clicked() {
                crate::updater::check_for_updates(self.updater.clone(), ctx.clone());
            }
        }
    }

    fn render_top_bar(&mut self, ui: &mut egui::Ui) {
        let mut target_to_load = None;

        ui.horizontal(|ui| {
            // App badge
            ui.label(
                RichText::new("ztrim")
                    .size(17.0)
                    .strong()
                    .color(ACCENT_GLOW),
            );
            ui.label(RichText::new("|").color(LINE));

            // Folder file navigation
            if let (Some(nav), Some(player)) = (&self.folder_nav, &self.player) {
                let cur_idx = nav.current_index(&player.file_path);
                let total_vids = nav.videos.len();

                // Prev button
                let has_prev = cur_idx.map(|i| i > 0).unwrap_or(false);
                if ui.add_enabled(has_prev, egui::Button::new("< Prev").min_size(vec2(52.0, 22.0))).clicked() {
                    if let Some(prev) = nav.prev_video(&player.file_path) {
                        target_to_load = Some(prev);
                    }
                }

                // File indicator
                let fname = player
                    .file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("video");
                let idx_str = cur_idx
                    .map(|i| format!("({}/{})", i + 1, total_vids))
                    .unwrap_or_default();

                ui.label(RichText::new(format!("{fname} {idx_str}")).size(13.0).strong().color(TEXT));

                // Next button
                let has_next = cur_idx.map(|i| i + 1 < total_vids).unwrap_or(false);
                if ui.add_enabled(has_next, egui::Button::new("Next >").min_size(vec2(52.0, 22.0))).clicked() {
                    if let Some(next) = nav.next_video(&player.file_path) {
                        target_to_load = Some(next);
                    }
                }

                // Skip button (Skip forward to next untrimmed video)
                let untrimmed_remaining = nav.untrimmed_count(&self.history);
                if ui.add_enabled(untrimmed_remaining > 0, egui::Button::new(RichText::new("Skip").size(11.5).color(AMBER))).clicked() {
                    if let Some(next_untrimmed) = nav.next_untrimmed(&player.file_path, &self.history) {
                        target_to_load = Some(next_untrimmed);
                    }
                }

                // Untrimmed / Processed status badge
                let is_trimmed = self.history.is_trimmed(&player.file_path);
                if is_trimmed {
                    let trimmed_cnt = nav.trimmed_count(&self.history);
                    ui.label(RichText::new(format!("Trimmed ({trimmed_cnt} total)")).size(11.0).color(GREEN));
                } else {
                    ui.label(RichText::new("Untrimmed").size(11.0).color(AMBER));
                }

                if untrimmed_remaining > 0 {
                    ui.label(RichText::new(format!("({} untrimmed left)", untrimmed_remaining)).size(10.5).color(DIM));
                }
            } else {
                ui.label(RichText::new("No video open. Drop a video or click Open File.").color(DIM));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.render_update_button(ui);

                // Windows Context Menu setup
                if cfg!(windows) {
                    let registered = is_context_menu_registered();
                    let menu_btn_txt = if registered {
                        "Context Menu: On"
                    } else {
                        "+ Add to Context Menu"
                    };
                    let menu_btn = egui::Button::new(
                        RichText::new(menu_btn_txt)
                            .size(11.0)
                            .color(if registered { GREEN } else { DIM }),
                    );
                    if ui.add(menu_btn).clicked() {
                        if registered {
                            let _ = unregister_context_menu();
                            self.set_status("Removed from Windows context menu".into());
                        } else {
                            match register_context_menu() {
                                Ok(_) => self.set_status("Added to Windows right-click context menu!".into()),
                                Err(e) => self.set_status(format!("Registry error: {e}")),
                            }
                        }
                    }
                }

                if ui.button("Open Folder...").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        let nav = FolderNavigator::from_file(&dir.join("placeholder.mp4"));
                        if nav.videos.is_empty() {
                            self.set_status("No video files found in folder".into());
                        } else if let Some(untrimmed) = nav.first_untrimmed(&self.history) {
                            let untrimmed_cnt = nav.untrimmed_count(&self.history);
                            self.set_status(format!("Opened folder: {} untrimmed clips", untrimmed_cnt));
                            target_to_load = Some(untrimmed);
                        } else {
                            self.prompt_open_trimmed = Some((dir, nav.videos.clone()));
                        }
                    }
                }

                if ui.button("Open File...").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Video Files", &["mp4", "mkv", "mov", "webm", "avi"])
                        .pick_file()
                    {
                        target_to_load = Some(path);
                    }
                }
            });
        });

        if let Some(p) = target_to_load {
            self.load_video_file(&p, true);
        }
    }

    fn render_canvas_and_timeline(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let player = match self.player.as_mut() {
            Some(p) => p,
            None => {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("Drag and drop a video file here").size(18.0).color(DIM));
                        ui.add_space(8.0);
                        ui.label(RichText::new("Or right-click any video in Windows and choose 'Trim & Compress with ztrim'").size(12.0).color(LINE));
                    });
                });
                return;
            }
        };

        player.update(ctx);

        let p_w = player.original_width;
        let p_h = player.original_height;

        // Canvas Area (Expands into area previously used by resolution header)
        let avail_size = ui.available_size();
        let canvas_area = vec2(avail_size.x, (avail_size.y - 95.0).max(120.0));
        let (canvas_rect, resp) = ui.allocate_exact_size(canvas_area, Sense::click_and_drag());

        let target_size = vec2(p_w as f32, p_h as f32);
        let fit_size = fit_aspect(target_size, canvas_area);

        // Zoom with mouse scroll (Min 100%, no zooming out below 100%)
        if resp.hovered() {
            let (scroll_steps, pinch) = ctx.input(|i| {
                let mut steps = 0.0f32;
                for ev in &i.raw.events {
                    if let egui::Event::MouseWheel { unit, delta, .. } = ev {
                        match unit {
                            egui::MouseWheelUnit::Line => steps += delta.y,
                            egui::MouseWheelUnit::Point => steps += delta.y / 60.0,
                            egui::MouseWheelUnit::Page => steps += delta.y * 3.0,
                        }
                    }
                }
                (steps, i.zoom_delta())
            });

            let mut factor = 1.0f32;
            if scroll_steps != 0.0 {
                factor *= 1.12f32.powf(scroll_steps.clamp(-4.0, 4.0));
            }
            if (pinch - 1.0).abs() > 0.001 {
                factor *= pinch;
            }
            if (factor - 1.0).abs() > 0.0005 {
                let old_z = self.canvas_zoom;
                let new_z = (old_z * factor).clamp(1.0, 10.0);
                if new_z <= 1.001 {
                    self.canvas_zoom = 1.0;
                    self.canvas_pan = vec2(0.0, 0.0);
                } else {
                    if let Some(m) = resp.hover_pos() {
                        let center = canvas_rect.center();
                        let rel = m - (center + self.canvas_pan);
                        self.canvas_pan -= rel * (new_z / old_z - 1.0);
                    }
                    self.canvas_zoom = new_z;
                }
            }
        }

        // Lock center to default position at 100% zoom (no scrolling / panning around)
        if self.canvas_zoom > 1.001 {
            if resp.dragged() {
                self.canvas_pan += resp.drag_delta();
            }
        } else {
            self.canvas_zoom = 1.0;
            self.canvas_pan = vec2(0.0, 0.0);
        }

        if resp.double_clicked() {
            self.canvas_zoom = 1.0;
            self.canvas_pan = vec2(0.0, 0.0);
        } else if resp.clicked() {
            player.toggle_play();
        }

        let cur_size = fit_size * self.canvas_zoom;
        let cur_center = canvas_rect.center() + self.canvas_pan;
        let image_rect = Rect::from_center_size(cur_center, cur_size);

        let p = ui.painter().with_clip_rect(canvas_rect);
        p.rect_filled(canvas_rect, 6.0, Color32::from_rgb(16, 16, 18));
        p.rect_stroke(canvas_rect, 6.0, Stroke::new(1.0, LINE), StrokeKind::Inside);

        if let Some(tex) = player.current_texture(ctx) {
            p.image(tex.id(), image_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }

        // Small indicator overlay when zoomed in
        if self.canvas_zoom > 1.001 {
            let zoom_badge_rect = Rect::from_min_size(
                Pos2::new(canvas_rect.right() - 88.0, canvas_rect.top() + 8.0),
                vec2(80.0, 20.0),
            );
            p.rect_filled(zoom_badge_rect, 4.0, Color32::from_rgba_unmultiplied(20, 20, 24, 210));
            p.rect_stroke(zoom_badge_rect, 4.0, Stroke::new(1.0, LINE), StrokeKind::Inside);
            p.text(
                zoom_badge_rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{:.0}% (Reset)", self.canvas_zoom * 100.0),
                egui::FontId::proportional(10.5),
                TEXT,
            );
        }

        ui.add_space(6.0);

        // Timeline Bar & Scrub Controls
        self.render_timeline_controls(ui);
    }

    fn render_timeline_controls(&mut self, ui: &mut egui::Ui) {
        let mut player = match self.player.take() {
            Some(p) => p,
            None => return,
        };

        card(ui, |ui| {
            ui.vertical(|ui| {
                let full_w = ui.available_width();
                let show_hint = !self.has_seen_trim_hint;
                let top_margin = if show_hint { 20.0 } else { 10.0 };
                let total_h = top_margin + 28.0;
                let (track_rect, scrub_resp) = ui.allocate_exact_size(vec2(full_w, total_h), Sense::click_and_drag());
                let p = ui.painter();

                let dur = player.duration_sec.max(0.001);
                let cur_s = player.current_sec;
                let trim_s = player.trim_start();
                let trim_e = player.trim_end();

                let start_ratio = (trim_s / dur).clamp(0.0, 1.0) as f32;
                let end_ratio = (trim_e / dur).clamp(0.0, 1.0) as f32;
                let trim_x_min = track_rect.left() + track_rect.width() * start_ratio;
                let trim_x_max = track_rect.left() + track_rect.width() * end_ratio;

                // Geometry
                let trim_track_y = track_rect.top() + top_margin;
                let trim_zone_bottom = trim_track_y + 4.0;
                let play_bar_top = trim_zone_bottom + 3.0; // Comfort padding to prevent accidental clicks
                let split_y = trim_zone_bottom + 1.0;

                // 1. Playback Bar (Thickest bar, 50% taller, bottom)
                let play_track_h = 15.0;
                let play_track_y = play_bar_top + (track_rect.bottom() - play_bar_top) * 0.5;
                let play_track_bg = Rect::from_center_size(
                    Pos2::new(track_rect.center().x, play_track_y),
                    vec2(track_rect.width(), play_track_h),
                );
                // Clear, distinct gray / off-white track color for the trimmed out portions
                // so user can easily see where to click across the whole bar
                let scrub_trimmed_color = Color32::from_rgb(82, 86, 98);
                let scrub_stroke_color = Color32::from_rgb(125, 131, 148);
                p.rect_filled(play_track_bg, 4.0, scrub_trimmed_color);
                p.rect_stroke(play_track_bg, 4.0, Stroke::new(1.0, scrub_stroke_color), StrokeKind::Inside);

                // Active section highlight in playback bar (kept trim section)
                let play_trim_rect = Rect::from_min_max(
                    Pos2::new(trim_x_min, play_track_y - play_track_h * 0.5),
                    Pos2::new(trim_x_max, play_track_y + play_track_h * 0.5),
                );
                p.rect_filled(play_trim_rect, 3.0, Color32::from_rgb(37, 99, 235));
                p.rect_stroke(play_trim_rect, 3.0, Stroke::new(1.0, Color32::from_rgb(96, 165, 250)), StrokeKind::Inside);

                // Hover indicator line when cursor is hovering over the scrub bar
                if scrub_resp.hovered() {
                    if let Some(pos) = scrub_resp.hover_pos() {
                        if pos.y >= play_bar_top {
                            p.line_segment(
                                [Pos2::new(pos.x, play_track_bg.top() - 1.0), Pos2::new(pos.x, play_track_bg.bottom() + 1.0)],
                                Stroke::new(1.5, Color32::from_rgba_unmultiplied(255, 255, 255, 140)),
                            );
                        }
                    }
                }

                // 2. Trim Bar (Top, thin line Vegas-style)
                // Guide line
                p.line_segment(
                    [Pos2::new(track_rect.left(), trim_track_y), Pos2::new(track_rect.right(), trim_track_y)],
                    Stroke::new(1.0, Color32::from_rgb(46, 46, 52)),
                );
                // Active trim highlight line
                p.line_segment(
                    [Pos2::new(trim_x_min, trim_track_y), Pos2::new(trim_x_max, trim_track_y)],
                    Stroke::new(3.0, ACCENT),
                );

                // Vegas Start Marker: Downward triangle tab + vertical line
                let tab_size = 6.0;
                let start_tri = vec![
                    Pos2::new(trim_x_min - tab_size, trim_track_y - 7.0),
                    Pos2::new(trim_x_min + tab_size, trim_track_y - 7.0),
                    Pos2::new(trim_x_min, trim_track_y),
                ];
                p.add(egui::Shape::convex_polygon(start_tri, BLUE, Stroke::new(1.0, Color32::WHITE)));
                p.line_segment(
                    [Pos2::new(trim_x_min, trim_track_y), Pos2::new(trim_x_min, track_rect.bottom())],
                    Stroke::new(1.5, BLUE),
                );

                // Vegas End Marker: Downward triangle tab + vertical line
                // Red flash animation: 2x 400ms flashes (0..400ms RED, 400..600ms AMBER, 600..1000ms RED)
                let (is_end_trim_flashing_red, should_repaint) =
                    is_end_trim_flashing_red_at(self.end_trim_error_flash_start, Instant::now());
                if should_repaint {
                    ui.ctx().request_repaint();
                } else if self.end_trim_error_flash_start.is_some() {
                    self.end_trim_error_flash_start = None;
                }

                let end_tri = vec![
                    Pos2::new(trim_x_max - tab_size, trim_track_y - 7.0),
                    Pos2::new(trim_x_max + tab_size, trim_track_y - 7.0),
                    Pos2::new(trim_x_max, trim_track_y),
                ];
                let end_marker_color = if is_end_trim_flashing_red {
                    Color32::from_rgb(255, 60, 60)
                } else {
                    AMBER
                };
                let end_tab_stroke = if is_end_trim_flashing_red {
                    Stroke::new(1.5, Color32::WHITE)
                } else {
                    Stroke::new(1.0, Color32::WHITE)
                };
                let end_line_stroke = if is_end_trim_flashing_red {
                    Stroke::new(2.5, Color32::from_rgb(255, 60, 60))
                } else {
                    Stroke::new(1.5, AMBER)
                };

                p.add(egui::Shape::convex_polygon(end_tri, end_marker_color, end_tab_stroke));
                p.line_segment(
                    [Pos2::new(trim_x_max, trim_track_y), Pos2::new(trim_x_max, track_rect.bottom())],
                    end_line_stroke,
                );

                // 3. Playhead (Vegas-style playback marker line passing over the scrub bar, no dot)
                let cur_ratio = (cur_s / dur).clamp(0.0, 1.0) as f32;
                let playhead_x = track_rect.left() + track_rect.width() * cur_ratio;
                let marker_cap = vec![
                    Pos2::new(playhead_x - 4.5, play_track_bg.top() - 4.0),
                    Pos2::new(playhead_x + 4.5, play_track_bg.top() - 4.0),
                    Pos2::new(playhead_x, play_track_bg.top()),
                ];
                p.add(egui::Shape::convex_polygon(marker_cap, Color32::WHITE, Stroke::NONE));
                p.line_segment(
                    [Pos2::new(playhead_x, play_track_bg.top() - 4.0), Pos2::new(playhead_x, play_track_bg.bottom() + 2.0)],
                    Stroke::new(2.0, Color32::WHITE),
                );

                // 4. Onboarding Tutorial Badges (First time launch)
                if show_hint {
                    let mut l_badge_center_x = trim_x_min.clamp(track_rect.left() + 48.0, track_rect.right() - 48.0);
                    let mut r_badge_center_x = trim_x_max.clamp(track_rect.left() + 48.0, track_rect.right() - 48.0);

                    if (r_badge_center_x - l_badge_center_x).abs() < 98.0 {
                        if l_badge_center_x > track_rect.left() + 52.0 {
                            l_badge_center_x -= 24.0;
                        }
                        if r_badge_center_x < track_rect.right() - 52.0 {
                            r_badge_center_x += 24.0;
                        }
                    }

                    let badge_y = track_rect.top() + 8.0;

                    // Left Mouse Button: Start Trim
                    let l_badge_rect = Rect::from_center_size(Pos2::new(l_badge_center_x, badge_y), vec2(90.0, 15.0));
                    p.rect_filled(l_badge_rect, 3.0, Color32::from_rgb(18, 25, 45));
                    p.rect_stroke(l_badge_rect, 3.0, Stroke::new(1.0, BLUE), StrokeKind::Inside);

                    let m_center_l = Pos2::new(l_badge_rect.left() + 9.0, l_badge_rect.center().y);
                    let m_rect_l = Rect::from_center_size(m_center_l, vec2(8.0, 11.0));
                    p.rect_filled(m_rect_l, 2.0, Color32::from_rgb(34, 40, 56));
                    p.rect_stroke(m_rect_l, 2.0, Stroke::new(1.0, Color32::WHITE), StrokeKind::Inside);
                    let l_btn = Rect::from_min_max(m_rect_l.min, Pos2::new(m_rect_l.center().x, m_rect_l.center().y));
                    p.rect_filled(l_btn, 1.0, BLUE);
                    p.text(
                        Pos2::new(l_badge_rect.left() + 17.0, l_badge_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        "L-Click: Start",
                        egui::FontId::proportional(10.0),
                        TEXT,
                    );

                    // Right Mouse Button: End Trim
                    let r_badge_rect = Rect::from_center_size(Pos2::new(r_badge_center_x, badge_y), vec2(90.0, 15.0));
                    p.rect_filled(r_badge_rect, 3.0, Color32::from_rgb(42, 32, 18));
                    p.rect_stroke(r_badge_rect, 3.0, Stroke::new(1.0, AMBER), StrokeKind::Inside);

                    let m_center_r = Pos2::new(r_badge_rect.left() + 9.0, r_badge_rect.center().y);
                    let m_rect_r = Rect::from_center_size(m_center_r, vec2(8.0, 11.0));
                    p.rect_filled(m_rect_r, 2.0, Color32::from_rgb(34, 40, 56));
                    p.rect_stroke(m_rect_r, 2.0, Stroke::new(1.0, Color32::WHITE), StrokeKind::Inside);
                    let r_btn = Rect::from_min_max(Pos2::new(m_rect_r.center().x, m_rect_r.min.y), Pos2::new(m_rect_r.max.x, m_rect_r.center().y));
                    p.rect_filled(r_btn, 1.0, AMBER);
                    p.text(
                        Pos2::new(r_badge_rect.left() + 17.0, r_badge_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        "R-Click: End",
                        egui::FontId::proportional(10.0),
                        TEXT,
                    );
                }

                let mut user_interacted_with_trim = false;

                // Stop dragging when mouse button is released
                let is_drag_stopped = scrub_resp.drag_stopped()
                    || scrub_resp.drag_stopped_by(egui::PointerButton::Secondary)
                    || !ui.input(|i| i.pointer.primary_down() || i.pointer.secondary_down());
                if is_drag_stopped {
                    if self.active_timeline_drag == TimelineDrag::Playhead {
                        if self.was_playing_before_scrub {
                            self.was_playing_before_scrub = false;
                            player.is_playing = true;
                            player.seek_to_time(player.current_sec);
                        } else if self.timeline_drag_occurred && !player.is_playing {
                            player.seek_to_time(player.current_sec);
                        }
                    } else if self.active_timeline_drag == TimelineDrag::TrimStart
                        || self.active_timeline_drag == TimelineDrag::TrimEnd
                    {
                        if let Some(initial) = self.trim_drag_initial.take() {
                            let current = TrimStateSnapshot {
                                start: player.trim_start_sec,
                                end: player.trim_end_sec,
                            };
                            if current != initial {
                                self.record_trim_undo(initial);
                            }
                        }
                    }
                    self.active_timeline_drag = TimelineDrag::None;
                    self.timeline_drag_occurred = false;
                }

                // Interaction 1: Right-Click logic
                // Right click always controls the end trim only (never start trim).
                // In main playback bar: does nothing.
                let right_clicked = scrub_resp.secondary_clicked()
                    || scrub_resp.drag_started_by(egui::PointerButton::Secondary)
                    || (scrub_resp.hovered() && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary) || i.pointer.button_clicked(egui::PointerButton::Secondary)));

                if right_clicked {
                    let origin_pos = ui.input(|i| i.pointer.press_origin())
                        .or_else(|| scrub_resp.interact_pointer_pos())
                        .or_else(|| scrub_resp.hover_pos())
                        .or_else(|| ui.input(|i| i.pointer.latest_pos()));

                    if let Some(pos) = origin_pos {
                        if pos.y < split_y {
                            let rel = ((pos.x - track_rect.left()) / track_rect.width()).clamp(0.0, 1.0) as f64;
                            let clicked_sec = rel * dur;
                            let decision = evaluate_trim_click(
                                true,
                                pos.x,
                                trim_x_min,
                                trim_x_max,
                                clicked_sec,
                                player.trim_start(),
                                player.trim_end(),
                                dur,
                            );

                            match decision {
                                TrimClickDecision::BlockRightClickFlashRed => {
                                    self.end_trim_error_flash_start = Some(Instant::now());
                                    ui.ctx().request_repaint();
                                    self.set_status("Blocked: Right-click moves End Trim only (minimum trim reached)".to_string());
                                }
                                TrimClickDecision::SetEndTrim(new_e) => {
                                    let snapshot = TrimStateSnapshot {
                                        start: player.trim_start_sec,
                                        end: player.trim_end_sec,
                                    };
                                    self.trim_drag_initial = Some(snapshot);
                                    player.trim_end_sec = Some(new_e);
                                    self.active_timeline_drag = TimelineDrag::TrimEnd;
                                    player.seek_to_time(new_e);
                                    self.set_status(format!(
                                        "Set End Trim: {:02}:{:04.1}",
                                        (new_e / 60.0) as u32, new_e % 60.0
                                    ));
                                    user_interacted_with_trim = true;
                                }
                                TrimClickDecision::SetStartTrim(_) => {}
                            }
                        }
                    }
                }

                // Interaction 2: Left-Click & Drag
                let left_clicked = scrub_resp.clicked()
                    || scrub_resp.drag_started()
                    || (scrub_resp.hovered() && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary) || i.pointer.button_clicked(egui::PointerButton::Primary)));

                if !right_clicked && self.active_timeline_drag == TimelineDrag::None && left_clicked {
                    let origin_pos = ui.input(|i| i.pointer.press_origin())
                        .or_else(|| scrub_resp.interact_pointer_pos())
                        .or_else(|| scrub_resp.hover_pos())
                        .or_else(|| ui.input(|i| i.pointer.latest_pos()));

                    if let Some(pos) = origin_pos {
                        let rel = ((pos.x - track_rect.left()) / track_rect.width()).clamp(0.0, 1.0) as f64;
                        let target_sec = rel * dur;

                        if pos.y < split_y {
                            let decision = evaluate_trim_click(
                                false,
                                pos.x,
                                trim_x_min,
                                trim_x_max,
                                target_sec,
                                player.trim_start(),
                                player.trim_end(),
                                dur,
                            );

                            let snapshot = TrimStateSnapshot {
                                start: player.trim_start_sec,
                                end: player.trim_end_sec,
                            };
                            self.trim_drag_initial = Some(snapshot);

                            match decision {
                                TrimClickDecision::SetEndTrim(new_e) => {
                                    player.trim_end_sec = Some(new_e);
                                    self.active_timeline_drag = TimelineDrag::TrimEnd;
                                    player.seek_to_time(new_e);
                                    self.set_status(format!(
                                        "Adjust End Trim: {:02}:{:04.1}",
                                        (new_e / 60.0) as u32, new_e % 60.0
                                    ));
                                    user_interacted_with_trim = true;
                                }
                                TrimClickDecision::SetStartTrim(new_s) => {
                                    player.trim_start_sec = Some(new_s);
                                    self.active_timeline_drag = TimelineDrag::TrimStart;
                                    player.seek_to_time(new_s);
                                    self.set_status(format!(
                                        "Adjust Start Trim: {:02}:{:04.1}",
                                        (new_s / 60.0) as u32, new_s % 60.0
                                    ));
                                    user_interacted_with_trim = true;
                                }
                                TrimClickDecision::BlockRightClickFlashRed => {}
                            }
                        } else {
                            // In main playback bar: moves scrub bar with instant response (like MPC-BE)
                            self.active_timeline_drag = TimelineDrag::Playhead;
                            self.timeline_drag_occurred = false;

                            if player.is_playing {
                                if scrub_resp.drag_started() {
                                    self.was_playing_before_scrub = true;
                                    player.pause();
                                    player.scrub_to_time(target_sec, true);
                                } else {
                                    // Direct click/jump: jump playback directly without pausing!
                                    self.was_playing_before_scrub = false;
                                    player.seek_to_time(target_sec);
                                }
                            } else {
                                self.was_playing_before_scrub = false;
                                player.seek_to_time(target_sec);
                            }
                        }
                    }
                }

                let is_dragged = scrub_resp.dragged()
                    || scrub_resp.dragged_by(egui::PointerButton::Secondary)
                    || (self.active_timeline_drag != TimelineDrag::None && ui.input(|i| i.pointer.primary_down() || i.pointer.secondary_down()));

                if is_dragged {
                    if let Some(pos) = scrub_resp.interact_pointer_pos().or_else(|| scrub_resp.hover_pos()).or_else(|| ui.input(|i| i.pointer.latest_pos())) {
                        let rel = ((pos.x - track_rect.left()) / track_rect.width()).clamp(0.0, 1.0) as f64;
                        let target_sec = rel * dur;

                        match self.active_timeline_drag {
                            TimelineDrag::TrimStart => {
                                self.timeline_drag_occurred = true;
                                let end_s = player.trim_end();
                                let new_s = target_sec.clamp(0.0, (end_s - 0.05).max(0.0));
                                player.trim_start_sec = Some(new_s);
                                player.scrub_to_time(new_s, false);
                                user_interacted_with_trim = true;
                            }
                            TimelineDrag::TrimEnd => {
                                self.timeline_drag_occurred = true;
                                let start_s = player.trim_start();
                                let new_e = target_sec.clamp((start_s + 0.05).min(dur), dur);
                                player.trim_end_sec = Some(new_e);
                                player.scrub_to_time(new_e, false);
                                user_interacted_with_trim = true;
                            }
                            TimelineDrag::Playhead => {
                                self.timeline_drag_occurred = true;
                                if player.is_playing {
                                    self.was_playing_before_scrub = true;
                                    player.pause();
                                }
                                player.scrub_to_time(target_sec, self.was_playing_before_scrub);
                            }
                            TimelineDrag::None => {}
                        }
                    }
                }

                if user_interacted_with_trim && !self.has_seen_trim_hint {
                    self.has_seen_trim_hint = true;
                    self.save_config();
                }

                ui.add_space(3.0);

                // Transport Controls Row
                ui.horizontal(|ui| {
                    let play_txt = if player.is_playing { "Pause" } else { "Play" };
                    let play_btn = egui::Button::new(RichText::new(play_txt).size(12.0).strong().color(Color32::WHITE))
                        .fill(if player.is_playing { ACCENT } else { CARD_HI })
                        .stroke(Stroke::new(1.0, LINE))
                        .corner_radius(6)
                        .min_size(vec2(52.0, 26.0));
                    if ui.add(play_btn).clicked() {
                        player.toggle_play();
                    }

                    if ui.button("< 1f").clicked() {
                        player.step_frames(-1);
                    }
                    if ui.button("1f >").clicked() {
                        player.step_frames(1);
                    }

                    ui.add_space(6.0);

                    // Set Start / Set End buttons
                    if ui.button("[ Set Start (I)").clicked() {
                        let snapshot = TrimStateSnapshot {
                            start: player.trim_start_sec,
                            end: player.trim_end_sec,
                        };
                        player.set_trim_start_to_current();
                        if player.trim_start_sec != snapshot.start {
                            self.record_trim_undo(snapshot);
                        }
                    }
                    if ui.button("Set End (O) ]").clicked() {
                        let snapshot = TrimStateSnapshot {
                            start: player.trim_start_sec,
                            end: player.trim_end_sec,
                        };
                        player.set_trim_end_to_current();
                        if player.trim_end_sec != snapshot.end {
                            self.record_trim_undo(snapshot);
                        }
                    }

                    if player.has_custom_trim() {
                        if ui.button("Reset Trim").clicked() {
                            let snapshot = TrimStateSnapshot {
                                start: player.trim_start_sec,
                                end: player.trim_end_sec,
                            };
                            player.reset_trim();
                            self.record_trim_undo(snapshot);
                            self.right_click_trim_anchor = None;
                            self.end_trim_error_flash_start = None;
                        }
                    }

                    ui.add_space(6.0);

                    // Time display
                    let time_str = format!(
                        "{:02}:{:04.1} / {:02}:{:04.1}",
                        (cur_s / 60.0) as u32, cur_s % 60.0,
                        (dur / 60.0) as u32, dur % 60.0
                    );
                    ui.label(RichText::new(time_str).size(12.0).monospace().color(TEXT));

                    if player.has_custom_trim() {
                        let trimmed_len = player.trimmed_duration();
                        let pct_saved = ((1.0 - (trimmed_len / dur)) * 100.0).clamp(0.0, 100.0);
                        let cut_info = format!("Trim: {:.1}s (-{:.0}%)", trimmed_len, pct_saved);
                        ui.label(RichText::new(cut_info).size(11.0).color(GREEN));
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let loop_txt = if player.loop_playback { "Loop: On" } else { "Loop: Off" };
                        if ui.button(loop_txt).clicked() {
                            player.loop_playback = !player.loop_playback;
                        }

                        if let Some(ref mut audio) = player.audio {
                            let mut vol = audio.volume;
                            if ui.add_sized([56.0, 14.0], egui::Slider::new(&mut vol, 0.0..=1.0).show_value(false)).changed() {
                                audio.set_volume(vol);
                            }
                            let mute_txt = if audio.is_muted { "Unmute" } else { "Mute" };
                            if ui.button(mute_txt).clicked() {
                                audio.toggle_mute();
                            }
                            ui.label(RichText::new("Vol:").size(11.0).color(DIM));
                        }
                    });
                });
            });
        });

        self.player = Some(player);
    }

    fn render_floating_undo_banner(&mut self, ui: &mut egui::Ui, anchor_y: f32, panel_left: f32, panel_width: f32) {
        let banner = match self.undo_banner.as_ref() {
            Some(b) => b.clone(),
            None => return,
        };

        // Detect whether undo is still possible in Windows Recycle Bin.
        // If the file was removed from the bin (or already undone), dismiss the banner immediately.
        let can_undo = match &banner.action {
            UndoAction::Trim { job_id } => {
                if let Ok(jobs) = self.render_mgr.active_jobs.lock() {
                    jobs.iter().find(|j| j.job_id == *job_id).map_or(false, |j| {
                        !j.is_undone
                            && j.backup_file_name
                                .as_ref()
                                .map_or(false, |bak| self.recycle_bin_watcher.is_file_in_bin(bak))
                    })
                } else {
                    false
                }
            }
            UndoAction::Delete { job_id } => {
                if let Ok(jobs) = self.render_mgr.active_jobs.lock() {
                    jobs.iter().find(|j| j.job_id == *job_id).map_or(false, |j| {
                        !j.is_undone && self.recycle_bin_watcher.is_file_in_bin(&j.file_name)
                    })
                } else {
                    false
                }
            }
        };

        if !can_undo {
            self.undo_banner = None;
            return;
        }

        // Calculate swipe-out animation progress p in [0.0, 1.0]
        let (p, expired) = if let Some(dt) = banner.dismissing_since {
            let el = dt.elapsed().as_secs_f32();
            if el >= UNDO_BANNER_SWIPE_SECS {
                (1.0, true)
            } else {
                ((el / UNDO_BANNER_SWIPE_SECS).clamp(0.0, 1.0), false)
            }
        } else {
            let el = banner.trigger_time.elapsed().as_secs_f32();
            if el >= UNDO_BANNER_TOTAL_DURATION_SECS {
                (1.0, true)
            } else if el >= UNDO_BANNER_HOLD_SECS {
                let swipe_el = el - UNDO_BANNER_HOLD_SECS;
                ((swipe_el / UNDO_BANNER_SWIPE_SECS).clamp(0.0, 1.0), false)
            } else {
                (0.0, false)
            }
        };

        if expired {
            self.undo_banner = None;
            return;
        }

        if p > 0.0 {
            ui.ctx().request_repaint();
        }

        // Ease-in curve ("slow to start, builds up speed until it's fully off screen" in 350ms)
        let ease_in = p * p * p;
        let exit_distance = panel_width + 40.0;
        let offset_x = exit_distance * ease_in;

        let banner_h = 38.0;
        let banner_top_y = anchor_y - 6.0 - banner_h;
        let banner_pos = Pos2::new(panel_left + offset_x, banner_top_y);

        let mut undo_triggered = false;
        let mut undo_dismissed = false;

        egui::Area::new(egui::Id::new("floating_undo_banner"))
            .order(egui::Order::Foreground)
            .fixed_pos(banner_pos)
            .interactable(p < 0.8)
            .show(ui.ctx(), |ui| {
                ui.set_width(panel_width);
                egui::Frame::new()
                    .fill(Color32::from_rgb(26, 26, 30))
                    .stroke(Stroke::new(1.5, ACCENT_GLOW))
                    .corner_radius(8)
                    .inner_margin(Margin::symmetric(10, 8))
                    .show(ui, |ui| {
                        ui.set_min_width(panel_width - 20.0);
                        ui.set_max_width(panel_width - 20.0);
                        ui.horizontal(|ui| {
                            // 1. Right-to-left layout for action controls (laid out first, prevents underlapping)
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let close_resp = ui.add(
                                    egui::Button::new(RichText::new("x").size(11.0).color(DIM))
                                        .frame(false)
                                        .min_size(vec2(16.0, 16.0)),
                                )
                                .on_hover_text("Dismiss");
                                if close_resp.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                                if close_resp.clicked() {
                                    undo_dismissed = true;
                                }

                                let undo_btn = egui::Button::new(
                                    RichText::new("Undo").size(11.5).strong().color(Color32::WHITE),
                                )
                                .fill(ACCENT)
                                .stroke(Stroke::new(1.0, ACCENT_GLOW))
                                .corner_radius(6)
                                .min_size(vec2(48.0, 22.0));

                                let undo_tip = match banner.action {
                                    UndoAction::Trim { .. } => "Restore original untrimmed video from Recycle Bin",
                                    UndoAction::Delete { .. } => "Restore deleted clip from Recycle Bin",
                                };

                                let u_resp = ui.add(undo_btn).on_hover_text(undo_tip);
                                if u_resp.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                                if u_resp.clicked() {
                                    undo_triggered = true;
                                }

                                // 2. Left-to-right in the remaining space for the tag and middle-truncated filename
                                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                    ui.label(RichText::new("Undo:").size(11.5).strong().color(AMBER));

                                    let max_w = (ui.available_width() - 8.0).max(10.0);
                                    let font_id = egui::FontId::proportional(11.5);
                                    let fitted_name = truncate_filename_apple_style(
                                        |s| ui.painter().layout_no_wrap(s.to_string(), font_id.clone(), Color32::WHITE).size().x,
                                        &banner.file_name,
                                        max_w,
                                    );

                                    let label_tip = format!("{}\nClick Undo to restore from Recycle Bin", banner.file_name);
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(fitted_name)
                                                .size(11.5)
                                                .strong()
                                                .color(TEXT),
                                        )
                                    )
                                    .on_hover_text(label_tip);
                                });
                            });
                        });
                    });
            });

        if undo_triggered {
            self.undo_current_banner();
        } else if undo_dismissed {
            if let Some(ref mut b) = self.undo_banner {
                if b.dismissing_since.is_none() {
                    b.dismissing_since = Some(Instant::now());
                    ui.ctx().request_repaint();
                }
            }
        }
    }

    fn render_right_pane(&mut self, ui: &mut egui::Ui) {
        let panel_left = ui.cursor().min.x;
        let panel_width = ui.available_width();
        let mut banner_anchor_y = None;

        // Bottom-anchored action bar so TRIM/COMPRESS and Prev/Skip stay anchored in the
        // bottom-right corner, directly adjacent to the end of the timeline scrub bar.
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            // Space from the bottom of the window matching the scrub bar's 18px total distance
            // (panel inner margin 10px + 8px space = 18px)
            ui.add_space(8.0);

            // Prev and Skip row
            ui.horizontal(|ui| {
                let has_nav = self.folder_nav.is_some() && self.player.is_some();
                if ui.add_enabled(has_nav, egui::Button::new("< Prev").min_size(vec2(68.0, 28.0))).clicked() {
                    self.go_prev();
                }

                let skip_btn = egui::Button::new(
                    RichText::new("Skip Video (Next Untrimmed)").strong().color(TEXT)
                )
                .min_size(vec2(ui.available_width(), 28.0));

                if ui.add_enabled(has_nav, skip_btn).clicked() {
                    self.skip_next();
                }
            });

            ui.add_space(6.0);

            // BIG Action Button (TRIM/COMPRESS)
            let can_render = self.player.is_some();
            let btn_text = self.action_button_text();
            let (rect, resp) = ui.allocate_exact_size(
                vec2(ui.available_width(), 44.0),
                if can_render { Sense::click() } else { Sense::hover() },
            );
            if can_render && resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if can_render && resp.clicked() {
                self.submit_current_job_and_advance();
            }

            let is_pressed = can_render && resp.is_pointer_button_down_on();
            let is_hovered = can_render && resp.hovered();

            let bg = if !can_render {
                Color32::from_rgb(28, 30, 36)
            } else if is_pressed {
                Color32::from_rgb(29, 78, 216)
            } else if is_hovered {
                Color32::from_rgb(45, 115, 255)
            } else {
                ACCENT
            };

            let border = if !can_render {
                Stroke::new(1.0, Color32::from_rgb(45, 47, 54))
            } else if is_hovered {
                Stroke::new(1.8, Color32::from_rgb(147, 197, 253))
            } else {
                Stroke::new(1.5, ACCENT_GLOW)
            };

            let text_color = if !can_render {
                DIM
            } else {
                Color32::WHITE
            };

            {
                let p = ui.painter();
                p.rect_filled(rect, 8.0, bg);
                p.rect_stroke(rect, 8.0, border, StrokeKind::Inside);

                let font_id = egui::FontId::new(15.0, egui::FontFamily::Name("button_font".into()));
                let text_w = p.layout_no_wrap(btn_text.to_string(), font_id.clone(), text_color).size().x;
                let icon_w = 16.0;
                let gap = 8.0;
                let total_w = icon_w + gap + text_w;
                let start_x = rect.center().x - total_w / 2.0;

                let icon_center = Pos2::new(start_x + icon_w / 2.0, rect.center().y);
                draw_scissors_icon(&p, icon_center, text_color);

                let text_pos = Pos2::new(start_x + icon_w + gap, rect.center().y);
                p.text(text_pos, egui::Align2::LEFT_CENTER, btn_text, font_id, text_color);
            }

            ui.add_space(6.0);

            // Delete Clip Button
            let can_delete = self.player.is_some();
            let (del_rect, del_resp) = ui.allocate_exact_size(
                vec2(ui.available_width(), 36.0),
                if can_delete { Sense::click() } else { Sense::hover() },
            );
            if can_delete && del_resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if can_delete && del_resp.clicked() {
                self.delete_current_clip();
            }

            let is_del_pressed = can_delete && del_resp.is_pointer_button_down_on();
            let is_del_hovered = can_delete && del_resp.hovered();

            let del_bg = if !can_delete {
                Color32::from_rgb(28, 30, 36)
            } else if is_del_pressed {
                Color32::from_rgb(153, 27, 27)
            } else if is_del_hovered {
                Color32::from_rgb(220, 38, 38)
            } else {
                Color32::from_rgb(185, 28, 28)
            };

            let del_border = if !can_delete {
                Stroke::new(1.0, Color32::from_rgb(45, 47, 54))
            } else if is_del_hovered {
                Stroke::new(1.5, Color32::from_rgb(248, 113, 113))
            } else {
                Stroke::new(1.0, Color32::from_rgb(239, 68, 68))
            };

            let del_text_color = if !can_delete {
                DIM
            } else {
                Color32::WHITE
            };

            {
                let p = ui.painter();
                p.rect_filled(del_rect, 8.0, del_bg);
                p.rect_stroke(del_rect, 8.0, del_border, StrokeKind::Inside);

                let del_font_id = egui::FontId::new(13.5, egui::FontFamily::Name("button_font".into()));
                let text_w = p.layout_no_wrap("DELETE CLIP".to_string(), del_font_id.clone(), del_text_color).size().x;
                let icon_w = 14.0;
                let gap = 7.0;
                let total_w = icon_w + gap + text_w;
                let start_x = del_rect.center().x - total_w / 2.0;

                let icon_center = Pos2::new(start_x + icon_w / 2.0, del_rect.center().y);
                draw_trash_icon(&p, icon_center, del_text_color);

                let text_pos = Pos2::new(start_x + icon_w + gap, del_rect.center().y);
                p.text(text_pos, egui::Align2::LEFT_CENTER, "DELETE CLIP", del_font_id, del_text_color);
            }

            ui.add_space(6.0);

            // Status message directly above the action buttons with padding so it never overlaps or bumps buttons
            let mut status_seen = false;
            if let Some((ref msg, time)) = self.status_message {
                if time.elapsed().as_secs() < 8 {
                    let resp = ui.add(egui::Label::new(RichText::new(msg).size(11.0).color(AMBER)).truncate())
                        .on_hover_text(msg);
                    banner_anchor_y = Some(resp.rect.top());
                    status_seen = true;
                    ui.add_space(6.0);
                }
            }

            let (sep_rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
            ui.painter().line_segment([sep_rect.left_center(), sep_rect.right_center()], Stroke::new(1.0, LINE));
            ui.add_space(6.0);

            if !status_seen {
                banner_anchor_y = Some(sep_rect.top());
            }

            // Remaining upper area flows top-down inside a scrollable area
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // Section 1: Mode Selection
                        card(ui, |ui| {
                            ui.horizontal(|ui| {
                                if ui.checkbox(&mut self.compress_enabled, "").changed() {
                                    self.save_config();
                                }
                                ui.label(
                                    RichText::new("Compress Video (NVIDIA NVENC VBR)")
                                        .size(13.0)
                                        .strong()
                                        .color(if self.compress_enabled { TEXT } else { DIM }),
                                );
                            });
                        });

                        ui.add_space(6.0);

                        // Section 2: Quality & Bitrate Settings (when Compress is enabled)
                        if self.compress_enabled {
                            card(ui, |ui| {
                                heading(ui, "QUALITY PRESET");

                                let mut new_preset_idx = None;

                                // Preset Dropdown
                                egui::ComboBox::from_id_salt("preset_dropdown")
                                    .width(ui.available_width() - 8.0)
                                    .selected_text(
                                        self.presets
                                            .get(self.selected_preset_idx)
                                            .map(|p| p.name.as_str())
                                            .unwrap_or("Custom"),
                                    )
                                    .show_ui(ui, |ui| {
                                        for (i, p) in self.presets.iter().enumerate() {
                                            if ui.selectable_value(&mut self.selected_preset_idx, i, &p.name).clicked() {
                                                new_preset_idx = Some(i);
                                            }
                                        }
                                    });

                                if let Some(idx) = new_preset_idx {
                                    self.sync_from_preset_idx(idx);
                                    self.save_config();
                                }

                                if self.is_auto_preset() {
                                    ui.add_space(6.0);
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("Auto Range:").size(11.5).color(TEXT));
                                        let mut level_idx = self.auto_quality_level.to_index();
                                        let slider = egui::Slider::new(&mut level_idx, 0..=2)
                                            .custom_formatter(|val, _| match val.round() as usize {
                                                0 => "Low".to_string(),
                                                1 => "Med".to_string(),
                                                _ => "High".to_string(),
                                            })
                                            .show_value(true);
                                        if ui.add(slider).changed() {
                                            self.auto_quality_level = AutoQualityLevel::from_index(level_idx);
                                            self.apply_auto_bitrate();
                                            self.save_config();
                                        }
                                    });
                                }

                                ui.add_space(8.0);
                                heading(ui, "VARIABLE BITRATE (VBR)");

                                // Average Bitrate
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Average Bitrate:").size(11.5).color(TEXT));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(RichText::new("Mb/s").size(11.0).color(DIM));
                                        ui.add(egui::DragValue::new(&mut self.avg_bitrate).range(1.0..=120.0).speed(0.2));
                                    });
                                });
                                ui.add(egui::Slider::new(&mut self.avg_bitrate, 1.0..=60.0).show_value(false));

                                ui.add_space(4.0);

                                // Maximum Bitrate
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Maximum Bitrate:").size(11.5).color(TEXT));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(RichText::new("Mb/s").size(11.0).color(DIM));
                                        ui.add(egui::DragValue::new(&mut self.max_bitrate).range(1.0..=200.0).speed(0.5));
                                    });
                                });
                                ui.add(egui::Slider::new(&mut self.max_bitrate, 2.0..=100.0).show_value(false));

                                // Keep max >= avg
                                if self.max_bitrate < self.avg_bitrate {
                                    self.max_bitrate = self.avg_bitrate;
                                }

                                ui.add_space(8.0);
                                heading(ui, "ENCODER & RESOLUTION");

                                // Encoder
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Codec:").size(11.5).color(TEXT));
                                    egui::ComboBox::from_id_salt("codec_select")
                                        .selected_text(self.selected_codec.display_name())
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(&mut self.selected_codec, VideoCodec::NvencHevc, VideoCodec::NvencHevc.display_name());
                                            ui.selectable_value(&mut self.selected_codec, VideoCodec::NvencH264, VideoCodec::NvencH264.display_name());
                                            ui.selectable_value(&mut self.selected_codec, VideoCodec::NvencAv1, VideoCodec::NvencAv1.display_name());
                                            ui.selectable_value(&mut self.selected_codec, VideoCodec::CpuX264, VideoCodec::CpuX264.display_name());
                                            ui.selectable_value(&mut self.selected_codec, VideoCodec::CpuX265, VideoCodec::CpuX265.display_name());
                                        });
                                });

                                // Resolution
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Target Res:").size(11.5).color(TEXT));
                                    let prev_res = self.selected_res;
                                    egui::ComboBox::from_id_salt("res_select")
                                        .selected_text(self.selected_res.display_name())
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(&mut self.selected_res, TargetResolution::Source, TargetResolution::Source.display_name());
                                            ui.selectable_value(&mut self.selected_res, TargetResolution::P1440, TargetResolution::P1440.display_name());
                                            ui.selectable_value(&mut self.selected_res, TargetResolution::P1080, TargetResolution::P1080.display_name());
                                            ui.selectable_value(&mut self.selected_res, TargetResolution::P4K, TargetResolution::P4K.display_name());
                                            ui.selectable_value(&mut self.selected_res, TargetResolution::P720, TargetResolution::P720.display_name());
                                        });
                                    if self.selected_res != prev_res {
                                        self.save_config();
                                        if self.is_auto_preset() {
                                            self.apply_auto_bitrate();
                                        }
                                    }
                                });

                                ui.add_space(6.0);
                                if ui.button("+ Save as New Preset...").clicked() {
                                    self.new_preset_modal_open = true;
                                }
                            });
                            ui.add_space(6.0);
                        }

                        // Section 3: Audio Normalisation
                        card(ui, |ui| {
                            let mut audio_toggled = false;
                            ui.horizontal(|ui| {
                                if ui.checkbox(&mut self.normalise_audio, "").changed() {
                                    audio_toggled = true;
                                }
                                ui.label(
                                    RichText::new("Normalise Audio Volume")
                                        .size(13.0)
                                        .strong()
                                        .color(if self.normalise_audio { TEXT } else { DIM }),
                                );
                            });

                            if audio_toggled {
                                self.save_config();
                                self.sync_player_audio_normalisation();
                            }

                            if self.normalise_audio {
                                ui.add_space(4.0);
                                if ui.checkbox(&mut self.normalise_audio_apply_to_export, "Apply to exported video").changed() {
                                    self.save_config();
                                }
                                ui.label(
                                    RichText::new("Unchecked = preview only")
                                        .size(10.0)
                                        .color(DIM),
                                );

                                ui.add_space(4.0);
                                let adv_icon = if self.audio_normalise_advanced_open { "v" } else { ">" };
                                let adv_resp = ui.add(
                                    egui::Button::new(
                                        RichText::new(format!("{} Advanced", adv_icon))
                                            .size(11.0)
                                            .color(ACCENT_GLOW),
                                    )
                                    .frame(false),
                                );
                                if adv_resp.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                                if adv_resp.clicked() {
                                    self.audio_normalise_advanced_open = !self.audio_normalise_advanced_open;
                                    self.save_config();
                                }

                                if self.audio_normalise_advanced_open {
                                    ui.add_space(6.0);

                                    let mut slider_changed = false;
                                    let mut drag_stopped = false;

                                    // Slider 1: Boost threshold
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("Boost Threshold:").size(11.5).color(TEXT));
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            ui.label(RichText::new("dB").size(11.0).color(DIM));
                                            let dv_resp = ui.add(egui::DragValue::new(&mut self.audio_normalise_thresh_db).range(-50.0..=-10.0).speed(0.5));
                                            if dv_resp.changed() {
                                                slider_changed = true;
                                            }
                                            if dv_resp.drag_stopped() || dv_resp.lost_focus() {
                                                drag_stopped = true;
                                            }
                                        });
                                    });
                                    let sl_resp = ui.add(egui::Slider::new(&mut self.audio_normalise_thresh_db, -50.0..=-10.0).show_value(false));
                                    if sl_resp.changed() {
                                        slider_changed = true;
                                    }
                                    if sl_resp.drag_stopped() {
                                        drag_stopped = true;
                                    }
                                    ui.label(RichText::new("Quiet audio below this volume is boosted to match louder audio").size(10.0).color(DIM));

                                    ui.add_space(6.0);

                                    // Slider 2: Amount to boost
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("Amount to Boost:").size(11.5).color(TEXT));
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            ui.label(RichText::new("dB").size(11.0).color(DIM));
                                            let dv_resp = ui.add(egui::DragValue::new(&mut self.audio_normalise_boost_db).range(0.0..=24.0).speed(0.5));
                                            if dv_resp.changed() {
                                                slider_changed = true;
                                            }
                                            if dv_resp.drag_stopped() || dv_resp.lost_focus() {
                                                drag_stopped = true;
                                            }
                                        });
                                    });
                                    let sl_resp = ui.add(egui::Slider::new(&mut self.audio_normalise_boost_db, 0.0..=24.0).show_value(false));
                                    if sl_resp.changed() {
                                        slider_changed = true;
                                    }
                                    if sl_resp.drag_stopped() {
                                        drag_stopped = true;
                                    }
                                    ui.label(RichText::new("Lifts quiet audio into audible range without clipping").size(10.0).color(DIM));

                                    if slider_changed {
                                        self.save_config();
                                    }
                                    if drag_stopped {
                                        self.sync_player_audio_normalisation();
                                    }

                                    ui.add_space(8.0);

                                    // Reset Defaults button
                                    let reset_btn = egui::Button::new(RichText::new("Reset Defaults").size(11.0).color(TEXT))
                                        .min_size(vec2(ui.available_width(), 22.0));
                                    if ui.add(reset_btn)
                                        .on_hover_text("Reset threshold to -25dB and boost to +10dB (ratio fixed at 6:1)")
                                        .clicked()
                                    {
                                        self.audio_normalise_ratio = 6.0;
                                        self.audio_normalise_thresh_db = -25.0;
                                        self.audio_normalise_boost_db = 10.0;
                                        self.save_config();
                                        self.sync_player_audio_normalisation();
                                    }
                                }
                            }
                        });

                        ui.add_space(6.0);

                        // Section 4: Workflow Options
                        card(ui, |ui| {
                            if ui.checkbox(&mut self.replace_in_place, "Replace video in-place (same filename)").changed() {
                                self.save_config();
                            }
                        });

                        ui.add_space(10.0);

                        // Persist history toggle directly above history section
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut self.persist_history, "Persist history")
                                .on_hover_text("Keep render queue history and undo availability across app launches")
                                .changed()
                            {
                                self.save_config();
                                if self.persist_history {
                                    self.render_mgr.save_queue_history();
                                } else {
                                    RenderManager::delete_queue_history_file();
                                }
                            }
                        });
                        ui.add_space(4.0);

                        // Section 4: Background Render Queue & History
                        heading(ui, "RENDER QUEUE & HISTORY");
                        let mut target_to_inspect = None;
                        let mut job_to_undo = None;
                        let mut job_to_remove = None;

                        card(ui, |ui| {
                            if let Ok(mut jobs) = self.render_mgr.active_jobs.lock() {
                                if jobs.is_empty() {
                                    ui.label(RichText::new("No render or trim history yet.").size(11.0).color(DIM));
                                } else {
                                    for j in jobs.iter_mut().rev() {
                                        if j.is_undone && j.undone_at.is_none() {
                                            j.undone_at = Some(Instant::now());
                                        }

                                        let (offset_x, should_remove) = if j.is_undone {
                                            let undone_since = j.undone_at.unwrap_or_else(Instant::now);
                                            let el = undone_since.elapsed().as_secs_f32();
                                            const UNDONE_HOLD_SECS: f32 = 0.8;
                                            const UNDONE_SWIPE_SECS: f32 = 0.350;
                                            if el >= UNDONE_HOLD_SECS + UNDONE_SWIPE_SECS {
                                                (0.0, true)
                                            } else if el >= UNDONE_HOLD_SECS {
                                                ui.ctx().request_repaint();
                                                let p = ((el - UNDONE_HOLD_SECS) / UNDONE_SWIPE_SECS).clamp(0.0, 1.0);
                                                let ease_in = p * p * p;
                                                let exit_distance = ui.available_width() + 50.0;
                                                (exit_distance * ease_in, false)
                                            } else {
                                                ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
                                                (0.0, false)
                                            }
                                        } else {
                                            (0.0, false)
                                        };

                                        if should_remove {
                                            job_to_remove = Some(j.job_id);
                                            continue;
                                        }

                                        let mut row_clicked = false;
                                        let (row_rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::hover());
                                        let shifted_rect = row_rect.translate(vec2(offset_x, 0.0));
                                        let mut row_ui = ui.new_child(egui::UiBuilder::new().max_rect(shifted_rect));

                                        row_ui.horizontal(|ui| {
                                            // 1. Right-to-left layout for action buttons & status (cuts off sooner, prevents overlap)
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                // Clear 'x' button (ASCII 'x', never an ASCII box!)
                                                let x_btn = egui::Button::new(RichText::new("x").size(11.0).color(DIM))
                                                    .frame(false)
                                                    .min_size(vec2(16.0, 16.0));
                                                let x_resp = ui.add(x_btn).on_hover_text("Clear this job from history");
                                                if x_resp.hovered() {
                                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                                }
                                                if x_resp.clicked() {
                                                    job_to_remove = Some(j.job_id);
                                                }

                                                // Detect whether undo is still possible in Windows Recycle Bin
                                                let can_undo = if !j.is_undone {
                                                    match &j.state {
                                                        JobState::Finished => {
                                                            j.backup_file_name
                                                                .as_ref()
                                                                .map_or(false, |bak| self.recycle_bin_watcher.is_file_in_bin(bak))
                                                        }
                                                        JobState::Deleted => {
                                                            self.recycle_bin_watcher.is_file_in_bin(&j.file_name)
                                                        }
                                                        _ => false,
                                                    }
                                                } else {
                                                    false
                                                };

                                                match &j.state {
                                                    JobState::Queued => {
                                                        ui.label(RichText::new("Queued").size(11.0).color(AMBER));
                                                    }
                                                    JobState::Running { progress, speed } => {
                                                        ui.label(
                                                            RichText::new(format!("{:.0}% ({speed})", progress * 100.0))
                                                                .size(11.0)
                                                                .color(BLUE),
                                                        );
                                                    }
                                                    JobState::Finished => {
                                                        if j.is_undone {
                                                            ui.label(RichText::new("Undone").size(10.5).color(DIM));
                                                        } else {
                                                            if can_undo {
                                                                let undo_btn = egui::Button::new(
                                                                    RichText::new("Undo").size(10.5).color(TEXT)
                                                                )
                                                                .fill(CARD_HI)
                                                                .stroke(Stroke::new(1.0, LINE))
                                                                .corner_radius(4)
                                                                .min_size(vec2(44.0, 18.0));

                                                                let u_resp = ui.add(undo_btn).on_hover_text("Restore original untrimmed video from Recycle Bin");
                                                                if u_resp.hovered() {
                                                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                                                }
                                                                if u_resp.clicked() {
                                                                    job_to_undo = Some(j.job_id);
                                                                }
                                                            }
                                                            ui.label(RichText::new("Done").size(11.0).color(GREEN));
                                                        }
                                                    }
                                                    JobState::Deleted => {
                                                        if j.is_undone {
                                                            ui.label(RichText::new("Undone").size(10.5).color(DIM));
                                                        } else {
                                                            if can_undo {
                                                                let undo_btn = egui::Button::new(
                                                                    RichText::new("Undo").size(10.5).color(TEXT)
                                                                )
                                                                .fill(CARD_HI)
                                                                .stroke(Stroke::new(1.0, LINE))
                                                                .corner_radius(4)
                                                                .min_size(vec2(44.0, 18.0));

                                                                let u_resp = ui.add(undo_btn).on_hover_text("Restore deleted clip from Recycle Bin");
                                                                if u_resp.hovered() {
                                                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                                                }
                                                                if u_resp.clicked() {
                                                                    job_to_undo = Some(j.job_id);
                                                                }
                                                            }
                                                            ui.label(RichText::new("Deleted").size(11.0).color(RED));
                                                        }
                                                    }
                                                    JobState::Failed(err) => {
                                                        ui.label(RichText::new("Failed").size(11.0).color(RED))
                                                            .on_hover_text(err);
                                                    }
                                                }

                                                // 2. Left-to-right in the remaining space for the middle-truncated filename
                                                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                                    let max_w = (ui.available_width() - 8.0).max(10.0);
                                                    let font_id = egui::FontId::proportional(11.5);
                                                    let fitted_name = truncate_filename_apple_style(
                                                        |s| ui.painter().layout_no_wrap(s.to_string(), font_id.clone(), Color32::WHITE).size().x,
                                                        &j.file_name,
                                                        max_w,
                                                    );

                                                    let label_resp = ui.add(
                                                        egui::Label::new(
                                                            RichText::new(fitted_name)
                                                                .size(11.5)
                                                                .strong()
                                                                .color(if j.is_undone { DIM } else { TEXT }),
                                                        )
                                                        .sense(Sense::click()),
                                                    )
                                                    .on_hover_text(format!("{}\nClick to load and view resulting clip in player", j.file_name));

                                                    if label_resp.hovered() {
                                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                                    }
                                                    if label_resp.clicked() {
                                                        row_clicked = true;
                                                    }
                                                });
                                            });
                                        });

                                        if row_clicked {
                                            target_to_inspect = Some(j.input_path.clone());
                                        }

                                        if let JobState::Running { progress, .. } = &j.state {
                                            ui.add(egui::ProgressBar::new(*progress).animate(true));
                                        }
                                        let (sep_rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                                        ui.painter().line_segment(
                                            [sep_rect.left_center() + vec2(offset_x, 0.0), sep_rect.right_center() + vec2(offset_x, 0.0)],
                                            Stroke::new(1.0, LINE),
                                        );
                                    }
                                }
                            }
                        });

                        if let Some(target) = target_to_inspect {
                            if target.exists() {
                                self.load_video_file(&target, false);
                                self.set_status(format!("Loaded: {}", target.file_name().unwrap_or_default().to_string_lossy()));
                            } else {
                                self.set_status("File not found on disk (may have been deleted or moved)".into());
                            }
                        }

                        if let Some(id) = job_to_undo {
                            self.undo_job(id);
                        }

                        if let Some(id) = job_to_remove {
                            self.render_mgr.remove_job(id);
                            if self.persist_history {
                                self.render_mgr.save_queue_history();
                            }
                        }
                    });
            });
        });

        if let Some(anchor_y) = banner_anchor_y {
            self.render_floating_undo_banner(ui, anchor_y, panel_left, panel_width);
        }
    }
}

fn fit_aspect(target: egui::Vec2, max_size: egui::Vec2) -> egui::Vec2 {
    let scale = (max_size.x / target.x.max(1.0)).min(max_size.y / target.y.max(1.0)).min(1.0);
    target * scale
}

fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(8)
        .inner_margin(Margin::symmetric(10, 8))
        .show(ui, add)
        .inner
}

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(11.0).strong().color(DIM));
    ui.add_space(2.0);
}

/// Truncates a filename in an Apple macOS Finder style:
/// Truncates the middle of the name out, leaving the last 3 characters of the filename stem + extension.
/// e.g. "HELLDIVERS 2 2024.03.21 - 01.01.35.06.DVR.mp4" -> "HELLDIVERS 2 202...DVR.mp4".
/// Under more constrained width:
/// - Prioritises the first 3 letters over the last 3 letters (shrinking back letters before first 3 letters).
/// - Prioritises the first letter over all including the file extension.
pub fn truncate_filename_apple_style(
    measure: impl Fn(&str) -> f32,
    filename: &str,
    max_w: f32,
) -> String {
    if measure(filename) <= max_w {
        return filename.to_string();
    }

    let (stem, ext) = if let Some(dot_idx) = filename.rfind('.') {
        if dot_idx > 0 {
            (&filename[..dot_idx], &filename[dot_idx..])
        } else {
            (filename, "")
        }
    } else {
        (filename, "")
    };

    let stem_chars: Vec<char> = stem.chars().collect();
    let total_stem = stem_chars.len();

    // If stem is too short to middle-truncate, fallback to graceful front preservation
    if total_stem <= 3 {
        let fn_chars: Vec<char> = filename.chars().collect();
        for keep in (1..fn_chars.len()).rev() {
            let candidate: String = fn_chars[..keep].iter().collect();
            if measure(&candidate) <= max_w {
                return candidate;
            }
        }
        return stem_chars.first().map(|c| c.to_string()).unwrap_or_default();
    }

    let back_len = 3.min(total_stem.saturating_sub(3));
    let back_chars: String = stem_chars[total_stem - back_len..].iter().collect();
    let max_front = total_stem - back_len;

    // Phase 1: Keep front (>= 3 chars) + "..." + last 3 of stem + ext
    // Binary search for maximum front length that fits within max_w
    let mut low = 3;
    let mut high = max_front;
    let mut best_phase1 = None;

    while low <= high {
        let mid = (low + high) / 2;
        let front: String = stem_chars[..mid].iter().collect();
        let candidate = format!("{front}...{back_chars}{ext}");
        if measure(&candidate) <= max_w {
            best_phase1 = Some(candidate);
            low = mid + 1;
        } else {
            if mid == 0 {
                break;
            }
            high = mid - 1;
        }
    }

    if let Some(cand) = best_phase1 {
        return cand;
    }

    // Phase 2: Prioritise first 3 letters over the last 3 letters.
    // Keep first 3 letters and shrink back characters (2 down to 1), then without back chars.
    let front_3: String = stem_chars[..3].iter().collect();
    for cur_back_len in (1..back_len).rev() {
        let back_sub: String = stem_chars[total_stem - cur_back_len..].iter().collect();
        let candidate = format!("{front_3}...{back_sub}{ext}");
        if measure(&candidate) <= max_w {
            return candidate;
        }
    }

    // Try front 3 + "..." + extension (without dot duplication)
    let ext_stripped = ext.strip_prefix('.').unwrap_or(ext);
    let candidate_ext = if !ext_stripped.is_empty() {
        format!("{front_3}...{ext_stripped}")
    } else {
        format!("{front_3}...")
    };
    if measure(&candidate_ext) <= max_w {
        return candidate_ext;
    }

    // Phase 3: Prioritise first letter over all including the file extension.
    for k in (1..=3.min(total_stem)).rev() {
        let f: String = stem_chars[..k].iter().collect();
        let candidate = format!("{f}...");
        if measure(&candidate) <= max_w {
            return candidate;
        }
    }

    // Absolute fallback: the first character
    stem_chars[0].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_config_serialization() {
        let mut cfg = AppConfig::default();
        cfg.compress_enabled = false;
        cfg.replace_in_place = true;
        cfg.selected_preset_idx = 2;
        cfg.last_file = Some(PathBuf::from("C:/Videos/test.mp4"));
        cfg.window_width = Some(1600.0);
        cfg.window_height = Some(900.0);
        cfg.window_maximized = true;
        cfg.normalise_audio = true;
        cfg.normalise_audio_apply_to_export = true;
        cfg.audio_normalise_ratio = 8.0;
        cfg.audio_normalise_thresh_db = -30.0;
        cfg.audio_normalise_boost_db = 14.0;
        cfg.audio_normalise_advanced_open = true;
        cfg.selected_res = TargetResolution::P1440;
        cfg.auto_quality_level = AutoQualityLevel::High;

        let json = serde_json::to_string(&cfg).expect("serialize");
        let loaded: AppConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(loaded.compress_enabled, false);
        assert_eq!(loaded.replace_in_place, true);
        assert_eq!(loaded.selected_preset_idx, 2);
        assert_eq!(loaded.last_file, Some(PathBuf::from("C:/Videos/test.mp4")));
        assert_eq!(loaded.window_width, Some(1600.0));
        assert_eq!(loaded.window_height, Some(900.0));
        assert_eq!(loaded.window_maximized, true);
        assert_eq!(loaded.persist_history, true);
        assert_eq!(loaded.normalise_audio, true);
        assert_eq!(loaded.normalise_audio_apply_to_export, true);
        assert_eq!(loaded.audio_normalise_ratio, 8.0);
        assert_eq!(loaded.audio_normalise_thresh_db, -30.0);
        assert_eq!(loaded.audio_normalise_boost_db, 14.0);
        assert_eq!(loaded.audio_normalise_advanced_open, true);
        assert_eq!(loaded.selected_res, TargetResolution::P1440);
        assert_eq!(loaded.auto_quality_level, AutoQualityLevel::High);

        // Test backward compatibility without last_file and window settings in json
        let old_json = r#"{"compress_enabled":true,"replace_in_place":false,"selected_preset_idx":1}"#;
        let old_cfg: AppConfig = serde_json::from_str(old_json).expect("deserialize old");
        assert_eq!(old_cfg.last_file, None);
        assert_eq!(old_cfg.window_width, None);
        assert_eq!(old_cfg.window_height, None);
        assert_eq!(old_cfg.window_maximized, false);
        assert_eq!(old_cfg.persist_history, true);
        assert_eq!(old_cfg.normalise_audio, false);
        assert_eq!(old_cfg.normalise_audio_apply_to_export, false);
        assert_eq!(old_cfg.audio_normalise_ratio, 6.0);
        assert_eq!(old_cfg.audio_normalise_thresh_db, -25.0);
        assert_eq!(old_cfg.audio_normalise_boost_db, 10.0);
        assert_eq!(old_cfg.audio_normalise_advanced_open, false);
        assert_eq!(old_cfg.selected_res, TargetResolution::Source);
        assert_eq!(old_cfg.auto_quality_level, AutoQualityLevel::Med);
    }

    #[test]
    fn test_window_geometry_tracking() {
        let mut cfg = AppConfig::default();
        assert_eq!(cfg.window_width, None);
        assert_eq!(cfg.window_height, None);
        assert_eq!(cfg.window_maximized, false);

        cfg.window_width = Some(1280.0);
        cfg.window_height = Some(720.0);
        cfg.window_maximized = true;
        assert_eq!(cfg.window_width.unwrap().max(920.0), 1280.0);
        assert_eq!(cfg.window_height.unwrap().max(580.0), 720.0);

        let cmd = egui::ViewportCommand::InnerSize(vec2(1280.0, 720.0));
        let _ = cmd;
    }

    #[test]
    fn test_left_click_trim_logic() {
        let trim_x_min = 100.0;
        let trim_x_max = 500.0;
        let cur_start = 10.0;
        let cur_end = 50.0;
        let dur = 100.0;

        // 1. Left click before start trim -> sets start trim
        let res = evaluate_trim_click(false, 50.0, trim_x_min, trim_x_max, 5.0, cur_start, cur_end, dur);
        assert_eq!(res, TrimClickDecision::SetStartTrim(5.0));

        // 2. Left click between start and end trim (well outside end padding) -> sets start trim
        let res = evaluate_trim_click(false, 300.0, trim_x_min, trim_x_max, 30.0, cur_start, cur_end, dur);
        assert_eq!(res, TrimClickDecision::SetStartTrim(30.0));

        // 3. Left click within 25px to the left of end marker (pos_x = 480.0, pad = 25px) -> grabs end trim
        let res = evaluate_trim_click(false, 480.0, trim_x_min, trim_x_max, 48.0, cur_start, cur_end, dur);
        assert_eq!(res, TrimClickDecision::SetEndTrim(48.0));

        // 4. Left click exactly at end marker -> grabs end trim
        let res = evaluate_trim_click(false, 500.0, trim_x_min, trim_x_max, 50.0, cur_start, cur_end, dur);
        assert_eq!(res, TrimClickDecision::SetEndTrim(50.0));

        // 5. Left click anywhere after end marker -> grabs end trim
        let res = evaluate_trim_click(false, 700.0, trim_x_min, trim_x_max, 70.0, cur_start, cur_end, dur);
        assert_eq!(res, TrimClickDecision::SetEndTrim(70.0));
    }

    #[test]
    fn test_start_trim_override_when_markers_close() {
        // Start marker at 100px, End marker at 105px (distance is 5px)
        let trim_x_min = 100.0;
        let trim_x_max = 105.0;
        let cur_start = 10.0;
        let cur_end = 10.5;
        let dur = 100.0;

        // Clicking at 101.0px:
        // Distance to start is 1.0px (<= 26.0px), distance to end is 4.0px.
        // Even though 101.0 is within 25px of end (105 - 25 = 80), start trim is closer!
        // Start trim's 26px padding overrides end trim padding for left clicks.
        let res = evaluate_trim_click(false, 101.0, trim_x_min, trim_x_max, 10.1, cur_start, cur_end, dur);
        assert_eq!(res, TrimClickDecision::SetStartTrim(10.1));

        // Clicking at 104.0px:
        // Distance to start is 4.0px, distance to end is 1.0px.
        // Closer to end trim, so end trim wins.
        let res = evaluate_trim_click(false, 104.0, trim_x_min, trim_x_max, 10.4, cur_start, cur_end, dur);
        assert_eq!(res, TrimClickDecision::SetEndTrim(10.4));

        // Clicking at 106.0px (after end marker):
        // Always grabs end trim marker.
        let res = evaluate_trim_click(false, 106.0, trim_x_min, trim_x_max, 10.6, cur_start, cur_end, dur);
        assert_eq!(res, TrimClickDecision::SetEndTrim(10.6));
    }

    #[test]
    fn test_right_click_trim_logic() {
        let trim_x_min = 100.0;
        let trim_x_max = 500.0;
        let cur_start = 10.0;
        let cur_end = 50.0;
        let dur = 100.0;

        // 1. Right click at valid location between start and end -> moves end trim only
        let res = evaluate_trim_click(true, 300.0, trim_x_min, trim_x_max, 30.0, cur_start, cur_end, dur);
        assert_eq!(res, TrimClickDecision::SetEndTrim(30.0));

        // 2. Right click after end marker -> moves end trim only
        let res = evaluate_trim_click(true, 700.0, trim_x_min, trim_x_max, 70.0, cur_start, cur_end, dur);
        assert_eq!(res, TrimClickDecision::SetEndTrim(70.0));

        // 3. Right click before start trim (pos_x < 100, sec < 10.0) -> blocks with red flash
        let res = evaluate_trim_click(true, 50.0, trim_x_min, trim_x_max, 5.0, cur_start, cur_end, dur);
        assert_eq!(res, TrimClickDecision::BlockRightClickFlashRed);

        // 4. Right click inside start trim marker area (pos_x <= 100 + 26.0) -> blocks with red flash
        let res = evaluate_trim_click(true, 115.0, trim_x_min, trim_x_max, 11.5, cur_start, cur_end, dur);
        assert_eq!(res, TrimClickDecision::BlockRightClickFlashRed);

        // 5. Right click exactly at start trim -> blocks with red flash
        let res = evaluate_trim_click(true, 100.0, trim_x_min, trim_x_max, 10.0, cur_start, cur_end, dur);
        assert_eq!(res, TrimClickDecision::BlockRightClickFlashRed);
    }

    #[test]
    fn test_end_trim_flash_timing() {
        use std::time::Duration;
        let start = Instant::now();

        // Flash 1: 0..400ms (RED)
        assert_eq!(is_end_trim_flashing_red_at(Some(start), start), (true, true));
        assert_eq!(is_end_trim_flashing_red_at(Some(start), start + Duration::from_millis(200)), (true, true));
        assert_eq!(is_end_trim_flashing_red_at(Some(start), start + Duration::from_millis(399)), (true, true));

        // Gap: 400..600ms (AMBER / normal)
        assert_eq!(is_end_trim_flashing_red_at(Some(start), start + Duration::from_millis(400)), (false, true));
        assert_eq!(is_end_trim_flashing_red_at(Some(start), start + Duration::from_millis(500)), (false, true));
        assert_eq!(is_end_trim_flashing_red_at(Some(start), start + Duration::from_millis(599)), (false, true));

        // Flash 2: 600..1000ms (RED)
        assert_eq!(is_end_trim_flashing_red_at(Some(start), start + Duration::from_millis(600)), (true, true));
        assert_eq!(is_end_trim_flashing_red_at(Some(start), start + Duration::from_millis(800)), (true, true));
        assert_eq!(is_end_trim_flashing_red_at(Some(start), start + Duration::from_millis(999)), (true, true));

        // Stopped: >= 1000ms
        assert_eq!(is_end_trim_flashing_red_at(Some(start), start + Duration::from_millis(1000)), (false, false));
        assert_eq!(is_end_trim_flashing_red_at(Some(start), start + Duration::from_millis(1500)), (false, false));

        // None
        assert_eq!(is_end_trim_flashing_red_at(None, start), (false, false));
    }

    #[test]
    fn test_truncate_filename_apple_style() {
        let measure_char_count = |s: &str| s.chars().count() as f32;
        let fn_full = "HELLDIVERS 2 2024.03.21 - 01.01.35.06.DVR.mp4";

        // Full fits
        assert_eq!(
            truncate_filename_apple_style(measure_char_count, fn_full, 100.0),
            fn_full
        );

        // Fits with middle truncated leaving last 3 characters of stem + .mp4 (Phase 1)
        let res25 = truncate_filename_apple_style(measure_char_count, fn_full, 25.0);
        assert!(res25.len() <= 25);
        assert!(res25.ends_with("...DVR.mp4") || res25.ends_with("DVR.mp4"));
        assert_eq!(res25, "HELLDIVERS 2 20...DVR.mp4");

        // Smallest Phase 1 candidate (first 3 letters + "..." + last 3 + .mp4 = 13 chars)
        let res13 = truncate_filename_apple_style(measure_char_count, fn_full, 13.0);
        assert_eq!(res13, "HEL...DVR.mp4");

        // Phase 2: Prioritise first 3 letters over last 3 letters (last 3 letters shrink)
        let res12 = truncate_filename_apple_style(measure_char_count, fn_full, 12.0);
        assert_eq!(res12, "HEL...VR.mp4");

        let res11 = truncate_filename_apple_style(measure_char_count, fn_full, 11.0);
        assert_eq!(res11, "HEL...R.mp4");

        let res10 = truncate_filename_apple_style(measure_char_count, fn_full, 10.0);
        assert_eq!(res10, "HEL...mp4");

        // Phase 3: Prioritise first letter over all including the file extension
        let res6 = truncate_filename_apple_style(measure_char_count, fn_full, 6.0);
        assert_eq!(res6, "HEL...");

        let res5 = truncate_filename_apple_style(measure_char_count, fn_full, 5.0);
        assert_eq!(res5, "HE...");

        let res4 = truncate_filename_apple_style(measure_char_count, fn_full, 4.0);
        assert_eq!(res4, "H...");

        let res1 = truncate_filename_apple_style(measure_char_count, fn_full, 1.0);
        assert_eq!(res1, "H");

        // No extension test
        let no_ext = "HelldiversMissionGameplay";
        let res_no_ext = truncate_filename_apple_style(measure_char_count, no_ext, 15.0);
        assert_eq!(res_no_ext, "Helldiver...lay");
    }

    #[test]
    fn test_trim_undo_redo_stack() {
        let mut app = ZTrimApp::new(None, None);
        assert!(app.trim_undo_stack.is_empty());
        assert!(app.trim_redo_stack.is_empty());

        // Without player, undo/redo returns false safely
        assert_eq!(app.undo_trim(), false);
        assert_eq!(app.redo_trim(), false);

        // Record some trim history
        let snap1 = TrimStateSnapshot { start: None, end: None };
        let snap2 = TrimStateSnapshot { start: Some(5.0), end: None };
        let snap3 = TrimStateSnapshot { start: Some(5.0), end: Some(15.0) };

        app.record_trim_undo(snap1);
        assert_eq!(app.trim_undo_stack.len(), 1);

        // Duplicate snapshot should not be pushed
        app.record_trim_undo(snap1);
        assert_eq!(app.trim_undo_stack.len(), 1);

        app.record_trim_undo(snap2);
        assert_eq!(app.trim_undo_stack.len(), 2);

        app.record_trim_undo(snap3);
        assert_eq!(app.trim_undo_stack.len(), 3);

        // Verify snapshots in order
        assert_eq!(app.trim_undo_stack[0], snap1);
        assert_eq!(app.trim_undo_stack[1], snap2);
        assert_eq!(app.trim_undo_stack[2], snap3);

        // Pre-populate redo stack and ensure record_trim_undo clears redo stack
        app.trim_redo_stack.push(snap2);
        assert_eq!(app.trim_redo_stack.len(), 1);
        let snap4 = TrimStateSnapshot { start: Some(6.0), end: Some(15.0) };
        app.record_trim_undo(snap4);
        assert!(app.trim_redo_stack.is_empty());
        assert_eq!(app.trim_undo_stack.len(), 4);
    }

    #[test]
    fn test_action_button_text_variations() {
        let mut app = ZTrimApp::new(None, None);
        app.player = None;

        // Compress enabled, no player
        app.compress_enabled = true;
        app.normalise_audio = false;
        assert_eq!(app.action_button_text(), "COMPRESS");

        // Compress disabled, normalise disabled
        app.compress_enabled = false;
        app.normalise_audio = false;
        app.normalise_audio_apply_to_export = false;
        assert_eq!(app.action_button_text(), "TRIM (LOSSLESS)");

        // Compress disabled, normalise enabled (preview only) -> still lossless export!
        app.compress_enabled = false;
        app.normalise_audio = true;
        app.normalise_audio_apply_to_export = false;
        assert_eq!(app.action_button_text(), "TRIM (LOSSLESS)");

        // Compress disabled, normalise enabled and applied to export, no player
        app.compress_enabled = false;
        app.normalise_audio = true;
        app.normalise_audio_apply_to_export = true;
        assert_eq!(app.action_button_text(), "NORMALISE AUDIO");
    }

    #[test]
    fn test_startup_with_persisted_jobs_has_no_stale_undo_banner() {
        use crate::render::{JobProgressInfo, JobState};

        let mut app = ZTrimApp::new(None, None);
        // Simulate loaded persisted jobs from history
        let sample = vec![
            JobProgressInfo {
                job_id: 10,
                file_name: "video1.mp4".into(),
                input_path: PathBuf::from("C:/video1.mp4"),
                backup_file_name: Some("video1.orig_bak_10.mp4".into()),
                state: JobState::Finished,
                finished_at: Some(Instant::now()),
                is_undone: false,
                undone_at: None,
            },
            JobProgressInfo {
                job_id: 11,
                file_name: "video2.mp4".into(),
                input_path: PathBuf::from("C:/video2.mp4"),
                backup_file_name: Some("video2.orig_bak_11.mp4".into()),
                state: JobState::Finished,
                finished_at: Some(Instant::now()),
                is_undone: false,
                undone_at: None,
            },
        ];
        app.render_mgr.load_persisted_jobs(sample);

        // Compute max job id exactly as done on startup
        let initial_max_job_id = app
            .render_mgr
            .active_jobs
            .lock()
            .map(|jobs| jobs.iter().map(|j| j.job_id).max().unwrap_or(0))
            .unwrap_or(0);
        app.last_banner_job_id = initial_max_job_id;

        assert_eq!(app.last_banner_job_id, 11);
        assert!(app.undo_banner.is_none());

        // Verify that running the completed jobs check loop does NOT trigger undo banner
        if let Ok(jobs) = app.render_mgr.active_jobs.lock() {
            for j in jobs.iter() {
                if j.state == JobState::Finished && j.job_id > app.last_banner_job_id {
                    app.last_banner_job_id = j.job_id;
                    if let Some(ref _backup_name) = j.backup_file_name {
                        app.undo_banner = Some(UndoBannerState {
                            file_name: j.file_name.clone(),
                            action: UndoAction::Trim { job_id: j.job_id },
                            trigger_time: Instant::now(),
                            dismissing_since: None,
                        });
                    }
                }
            }
        }

        // Banner MUST remain None after check
        assert!(app.undo_banner.is_none());
        assert_eq!(app.last_banner_job_id, 11);
    }

    #[test]
    fn test_undo_banner_duration_10_seconds() {
        use std::time::Duration;

        assert_eq!(UNDO_BANNER_TOTAL_DURATION_SECS, 10.0);
        assert_eq!(UNDO_BANNER_SWIPE_SECS, 0.350);

        let now = Instant::now();
        // At 9.0s elapsed: still in hold period, not swiping yet
        let state_active = UndoBannerState {
            file_name: "test.mp4".into(),
            action: UndoAction::Trim { job_id: 1 },
            trigger_time: now - Duration::from_secs_f32(9.0),
            dismissing_since: None,
        };
        let el = state_active.trigger_time.elapsed().as_secs_f32();
        assert!(el < UNDO_BANNER_TOTAL_DURATION_SECS);
        assert!(el < UNDO_BANNER_HOLD_SECS);

        // At 9.825s elapsed: in swipe-out phase (halfway through 350ms)
        let state_swiping = UndoBannerState {
            file_name: "test.mp4".into(),
            action: UndoAction::Trim { job_id: 1 },
            trigger_time: now - Duration::from_secs_f32(9.65 + 0.175),
            dismissing_since: None,
        };
        let el_swiping = state_swiping.trigger_time.elapsed().as_secs_f32();
        assert!(el_swiping >= UNDO_BANNER_HOLD_SECS);
        assert!(el_swiping < UNDO_BANNER_TOTAL_DURATION_SECS);
        let p = ((el_swiping - UNDO_BANNER_HOLD_SECS) / UNDO_BANNER_SWIPE_SECS).clamp(0.0, 1.0);
        assert!((p - 0.5).abs() < 0.05);
        let ease_in = p * p * p;
        // Ease in starts slow: at 50% time, progress is only 12.5% of the distance!
        assert!((ease_in - 0.125).abs() < 0.05);

        // At 10.1s elapsed: fully expired
        let state_expired = UndoBannerState {
            file_name: "test.mp4".into(),
            action: UndoAction::Trim { job_id: 1 },
            trigger_time: now - Duration::from_secs_f32(10.1),
            dismissing_since: None,
        };
        let elapsed_expired = state_expired.trigger_time.elapsed().as_secs_f32();
        assert!(elapsed_expired >= UNDO_BANNER_TOTAL_DURATION_SECS);
    }

    #[test]
    fn test_default_auto_preset_and_export() {
        let cfg = AppConfig::default();
        // Default preset must be Auto (index 0)
        assert_eq!(cfg.selected_preset_idx, 0);
        // Default export must be Original (Keep Source)
        assert_eq!(cfg.selected_res, TargetResolution::Source);
        // Default auto quality level is Med
        assert_eq!(cfg.auto_quality_level, AutoQualityLevel::Med);

        let mut app = ZTrimApp::new(None, None);
        app.sync_from_preset_idx(0);
        assert!(app.is_auto_preset());
        assert_eq!(app.presets[0].id, "auto");
        assert_eq!(app.presets[0].resolution, TargetResolution::Source);
    }

    #[test]
    fn test_undone_job_swipes_out_timing() {
        use std::time::Duration;

        let now = Instant::now();
        const UNDONE_HOLD_SECS: f32 = 0.8;
        const UNDONE_SWIPE_SECS: f32 = 0.350;

        // At 0.4s: hold phase, offset_x = 0.0, not expired
        let undone_400ms = now - Duration::from_secs_f32(0.4);
        let el = undone_400ms.elapsed().as_secs_f32();
        assert!(el < UNDONE_HOLD_SECS);

        // At 0.975s: half-way through swipe (800ms + 175ms)
        let undone_975ms = now - Duration::from_secs_f32(0.8 + 0.175);
        let el_swipe = undone_975ms.elapsed().as_secs_f32();
        assert!(el_swipe >= UNDONE_HOLD_SECS);
        assert!(el_swipe < UNDONE_HOLD_SECS + UNDONE_SWIPE_SECS);
        let p = ((el_swipe - UNDONE_HOLD_SECS) / UNDONE_SWIPE_SECS).clamp(0.0, 1.0);
        assert!((p - 0.5).abs() < 0.05);
        let ease_in = p * p * p;
        assert!((ease_in - 0.125).abs() < 0.05);

        // At 1.2s: fully expired
        let undone_1200ms = now - Duration::from_secs_f32(1.2);
        let el_expired = undone_1200ms.elapsed().as_secs_f32();
        assert!(el_expired >= UNDONE_HOLD_SECS + UNDONE_SWIPE_SECS);
    }

    #[test]
    fn test_setup_custom_fonts() {
        let ctx = egui::Context::default();
        setup_custom_fonts(&ctx);
        let mut output = ctx.run_ui(Default::default(), |_| {});
        output.textures_delta.clear();
        let font_id = egui::FontId::new(15.0, egui::FontFamily::Name("button_font".into()));
        let galley = ctx.fonts_mut(|f| f.layout_no_wrap("TRIM / COMPRESS".into(), font_id, Color32::WHITE));
        assert!(galley.size().x > 10.0);
    }
}

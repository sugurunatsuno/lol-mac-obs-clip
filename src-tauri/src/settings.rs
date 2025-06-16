use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::RecordingMode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub recording_mode: RecordingMode,
    pub segment_seconds: u32,
    pub fps: u32,
    pub video_source: String,
    pub audio_source: String,
    pub wrap_count: u32,
    pub bitrate: String,
    #[serde(default = "default_save_dir")]
    pub save_dir: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            recording_mode: RecordingMode::Obs,
            segment_seconds: 6,
            fps: 30,
            video_source: "1".into(),
            audio_source: "none".into(),
            wrap_count: 11,
            bitrate: "20M".into(),
            save_dir: default_save_dir(),
        }
    }
}

fn default_save_dir() -> String {
    use std::path::PathBuf;
    let mut dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("Movies");
    dir.to_string_lossy().to_string()
}

pub fn load_settings(path: &Path) -> Option<AppSettings> {
    fs::read_to_string(path).ok().and_then(|c| serde_json::from_str(&c).ok())
}

pub fn save_settings(path: &Path, settings: &AppSettings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(settings).unwrap())
}

#[derive(Clone)]
pub struct SettingsPath(pub PathBuf);

pub struct SettingsState(pub std::sync::Arc<std::sync::Mutex<AppSettings>>);

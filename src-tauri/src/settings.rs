//! 設定ファイルの読み書きを担当するモジュール
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
// 設定ファイルの保存先などに利用

use crate::RecordingMode;

#[derive(Debug, Clone, Serialize, Deserialize)]
/// アプリの各種設定を保持する構造体
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
// アプリ起動時の基本設定をまとめる

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

/// OS ごとのデフォルト保存先パスを返す
fn default_save_dir() -> String {
    use std::path::PathBuf;
    // ユーザーのホーム配下 Movies をデフォルトとする
    let mut dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("Movies");
    dir.to_string_lossy().to_string()
}

pub fn load_settings(path: &Path) -> Option<AppSettings> {
    // JSON を読み込んで構造体へ変換
    fs::read_to_string(path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
}

pub fn save_settings(path: &Path, settings: &AppSettings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    // 整形した JSON で書き出す
    // 書き込み処理を実行
    fs::write(path, serde_json::to_string_pretty(settings).unwrap())
}

#[derive(Clone)]
pub struct SettingsPath(pub PathBuf);
// 設定ファイルのパスを保持するだけのラッパー

pub struct SettingsState(pub std::sync::Arc<std::sync::Mutex<AppSettings>>);

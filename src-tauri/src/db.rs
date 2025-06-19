use crate::lol::LolEvent;
use crate::timesync::{TimeSnapshot, TimedEvent};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
// ファイル操作やパス操作に必要なクレートを読み込む

#[derive(Clone)]
/// Wrapper to keep compatibility with previous DB path usage
pub struct DbPath(pub PathBuf);
// パスを保持するだけのシンプルなラッパー

/// Initialize storage directory. This previously created an SQLite DB but now
/// just ensures the directory exists.
pub async fn init_db(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        // 親ディレクトリが無ければ作成する
        fs::create_dir_all(parent)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Write clip metadata as a JSON file next to the video file.
pub async fn write_clip_metadata(
    _db_path: &Path,
    video_path: &Path,
    time_map: &[TimeSnapshot],
    events: &[TimedEvent],
    clip_start: f64,
) -> Result<(), String> {
    let out_path = video_path.with_extension("json");
    let events_with_offset: Vec<EventWithOffset> = events
        .iter()
        .map(|ev| EventWithOffset {
            event: ev.event.clone(),
            real_time: ev.real_time,
            offset: ev.real_time - clip_start,
        })
        .collect();
    let meta = ClipMetadata {
        events: events_with_offset,
        time_map: time_map.to_vec(),
    };
    let json = serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?;
    fs::write(out_path, json).await.map_err(|e| e.to_string())
}

#[derive(Serialize, Deserialize)]
pub struct EventWithOffset {
    #[serde(flatten)]
    pub event: LolEvent,
    /// Absolute real time (seconds since Unix epoch)
    pub real_time: f64,
    pub offset: f64,
}

#[derive(Serialize, Deserialize)]
pub struct ClipMetadata {
    pub events: Vec<EventWithOffset>,
    pub time_map: Vec<TimeSnapshot>,
}

// クリップ内でのイベント発生位置を保持

/// Read clip metadata from the JSON file written by `write_clip_metadata`.
pub async fn read_clip_metadata(_db_path: &Path, path: &Path) -> Result<ClipMetadata, String> {
    let json_path = path.with_extension("json");
    let contents = fs::read_to_string(json_path)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&contents).map_err(|e| e.to_string())
}

/// Stubbed implementation kept for compatibility.
pub async fn list_clips(_db_path: &Path) -> Result<Vec<(String, String, u64)>, String> {
    // 現在は DB を利用していないため空配列を返す
    Ok(Vec::new())
}

/// No-op cleanup; previously removed orphan DB entries.
pub async fn cleanup_orphan_clips(_db_path: &Path) -> Result<(), String> {
    // 互換性のために残されたダミー実装
    Ok(())
}

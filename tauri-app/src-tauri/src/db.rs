use crate::lol::LolEvent;
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
    events: &[LolEvent],
    clip_start: f64,
) -> Result<(), String> {
    // 出力ファイル名を json に変換
    let out_path = video_path.with_extension("json");
    let events_with_offset: Vec<EventWithOffset> = events
        .iter()
        .cloned()
        .map(|ev| EventWithOffset {
            // 各イベントにオフセットを付与
            event: ev.clone(),
            offset: ev.EventTime - clip_start,
        })
        .collect();
    // JSON 文字列へ変換
    let json = serde_json::to_string_pretty(&events_with_offset).map_err(|e| e.to_string())?;
    // ファイルへ書き出し
    fs::write(out_path, json).await.map_err(|e| e.to_string())
}

#[derive(Serialize, Deserialize)]
pub struct EventWithOffset {
    #[serde(flatten)]
    pub event: LolEvent,
    pub offset: f64,
}
// クリップ内でのイベント発生位置を保持

/// Read clip metadata from the JSON file written by `write_clip_metadata`.
pub async fn get_clip_events(_db_path: &Path, path: &Path) -> Result<Vec<EventWithOffset>, String> {
    // JSON ファイルを開き内容を文字列で取得
    let json_path = path.with_extension("json");
    let contents = fs::read_to_string(json_path)
        .await
        .map_err(|e| e.to_string())?;
    // JSON を構造体へ変換
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

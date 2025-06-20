//! アプリ全体のコマンドや状態管理を行うメインモジュール
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{async_runtime::spawn, Manager};
use tokio::process::Command;
use tokio::sync::Mutex as AsyncMutex;
use tauri_plugin_notification::NotificationExt;
use flexi_logger::{Duplicate, FileSpec, Logger};
use log::{debug, error, info, trace, warn};

mod db;
mod ffmpeg; // ffmpeg 管理
mod lol; // LoL API ラッパー
mod obs; // OBS WebSocket クライアント
mod settings; // 設定関連 // SQLite アクセス

use db::{cleanup_orphan_clips, get_clip_events, init_db, list_clips, DbPath};
use ffmpeg::{write_clip_metadata, FfmpegProcess, FfmpegState, SharedFfmpegProcess};
use lol::{AllGameData, LolEvent, Player};
use obs::{send_obs_command_wrapper, set_record_directory, ObsWsState, SharedObsWsClient};
use settings::{load_settings, save_settings, AppSettings, SettingsPath, SettingsState}; // DB 操作用

fn init_logging() {
    let mut dir = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("lol_clip_tool");
    std::fs::create_dir_all(&dir).ok();
    Logger::try_with_env_or_str("info")
        .unwrap()
        .log_to_file(FileSpec::default().directory(dir))
        .duplicate_to_stdout(Duplicate::All)
        .format(flexi_logger::detailed_format)
        .start()
        .unwrap();
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
enum GameState {
    NotStarted,
    InProgress,
    Finished,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
enum ObsState {
    Disconnected,
    Recording,
    NotRecording,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordingMode {
    Obs,
    Shell,
}

#[derive(Debug, Clone, Serialize)]
/// フロントエンドへ返すアプリの状態を保持する構造体
struct AppStatus {
    game_state: GameState,
    obs_state: ObsState,
    recording_mode: RecordingMode,
    is_recording: bool,
    replay_buffer_running: bool,
}

#[derive(Debug, Clone, Serialize)]
struct GamePlayer {
    name: String,
    champion: String,
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// Shows a desktop notification.
///
/// When called as a Tauri command the [`AppHandle`] parameter is automatically
/// supplied by the runtime so the JS side does not pass it.
///
/// If you want to trigger a notification from Rust code where the `AppHandle`
/// is not available, call [`show_notification_no_handle`] instead.
#[tauri::command]
fn show_notification(app: tauri::AppHandle, title: String, body: String) -> Result<(), String> {
    show_notification_impl(Some(app), &title, &body)
}

/// Alternative notification function that does not require an [`AppHandle`].
/// This falls back to `notify_rust` so the icon may differ from the Tauri
/// plugin version.
pub fn show_notification_no_handle(title: &str, body: &str) -> Result<(), String> {
    show_notification_impl(None, title, body)
}

fn show_notification_impl(app: Option<tauri::AppHandle>, title: &str, body: &str) -> Result<(), String> {
    if let Some(handle) = app {

        info!("Showing notification: {} - {}", title, body);
        handle
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show()
            .map_err(|e| e.to_string())
    } else {

        info!("Showing notification without AppHandle: {} - {}", title, body);
        notify_rust::Notification::new()
            .summary(title)
            .body(body)
            .show()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}


#[tauri::command]
fn log_message(level: Option<String>, message: String) -> Result<(), String> {
    match level.as_deref().unwrap_or("info") {
        "error" => error!("{}", message),
        "warn" | "warning" => warn!("{}", message),
        "debug" => debug!("{}", message),
        "trace" => trace!("{}", message),
        _ => info!("{}", message),
    }
    Ok(())
}

#[tauri::command]
fn get_status(state: tauri::State<AppStatusState>) -> AppStatus {
    state.0.lock().unwrap().clone()
}

#[tauri::command]
fn get_current_players(state: tauri::State<LolDataState>) -> Vec<GamePlayer> {
    let lock = state.0.lock().unwrap();
    if let Some(ref data) = *lock {
        data.allPlayers
            .iter()
            .map(|p| GamePlayer {
                name: p.summonerName.clone(),
                champion: p.championName.clone(),
            })
            .collect()
    } else {
        Vec::new()
    }
}

#[tauri::command]
async fn set_recording_mode(
    state: tauri::State<'_, AppStatusState>,
    mode: RecordingMode,
) -> Result<(), String> {
    let mut lock = state.0.lock().unwrap();
    lock.recording_mode = mode;

    info!("Recording mode set to: {:?}", lock.recording_mode);
    Ok(())
}

#[tauri::command]
async fn start_recording(
    state: tauri::State<'_, AppStatusState>,
    obs_state: tauri::State<'_, ObsWsState>,
    ffmpeg_state: tauri::State<'_, FfmpegState>,
) -> Result<(), String> {
    // ゲーム開始時に呼び出され、録画処理を開始する
    let mode = {
        let mut lock = state.0.lock().unwrap();
        lock.game_state = GameState::InProgress;
        lock.obs_state = ObsState::Recording;
        lock.is_recording = true;
        lock.replay_buffer_running = false;
        lock.recording_mode.clone()
    };
    match mode {
        RecordingMode::Obs => {
            spawn(send_obs_command_wrapper(obs_state.0.clone(), "StartRecord")); // OBS 録画開始
            Ok(())
        }
        RecordingMode::Shell => {
            // ffmpeg を用いた録画を開始
            let shared = ffmpeg_state.0.clone();
            let mut proc = shared.lock().await;
            proc.start(shared.clone()).await
        }
    }
}

#[tauri::command]
async fn stop_recording(
    state: tauri::State<'_, AppStatusState>,
    obs_state: tauri::State<'_, ObsWsState>,
    ffmpeg_state: tauri::State<'_, FfmpegState>,
) -> Result<(), String> {
    // 録画を終了し状態をリセット
    let mode = {
        let mut lock = state.0.lock().unwrap();
        lock.game_state = GameState::Finished;
        lock.obs_state = ObsState::NotRecording;
        lock.is_recording = false;
        lock.replay_buffer_running = false;
        lock.recording_mode.clone()
    };
    match mode {
        RecordingMode::Obs => {
            spawn(send_obs_command_wrapper(obs_state.0.clone(), "StopRecord")); // OBS 停止
            Ok(())
        }
        RecordingMode::Shell => {
            let mut proc = ffmpeg_state.0.lock().await;
            proc.stop().await
        }
    }
}

#[tauri::command]
async fn start_replay_buffer(
    state: tauri::State<'_, AppStatusState>,
    obs_state: tauri::State<'_, ObsWsState>,
    ffmpeg_state: tauri::State<'_, FfmpegState>,
) -> Result<(), String> {
    // リプレイバッファ機能を開始する
    let mode = {
        let mut lock = state.0.lock().unwrap();
        lock.is_recording = true;
        lock.replay_buffer_running = true;
        lock.recording_mode.clone()
    };
    match mode {
        RecordingMode::Obs => {
            spawn(send_obs_command_wrapper(
                obs_state.0.clone(),
                "StartReplayBuffer",
            )); // OBS 側でリプレイ開始
            Ok(())
        }
        RecordingMode::Shell => {
            // ffmpeg バッファを起動
            let shared = ffmpeg_state.0.clone();
            let mut proc = shared.lock().await;
            proc.start(shared.clone()).await
        }
    }
}

#[tauri::command]
async fn stop_replay_buffer(
    state: tauri::State<'_, AppStatusState>,
    obs_state: tauri::State<'_, ObsWsState>,
    ffmpeg_state: tauri::State<'_, FfmpegState>,
) -> Result<(), String> {
    // リプレイバッファを停止する
    let mode = {
        let mut lock = state.0.lock().unwrap();
        lock.is_recording = false;
        lock.replay_buffer_running = false;
        lock.recording_mode.clone()
    };
    match mode {
        RecordingMode::Obs => {
            spawn(send_obs_command_wrapper(
                obs_state.0.clone(),
                "StopReplayBuffer",
            )); // OBS リプレイ停止
            Ok(())
        }
        RecordingMode::Shell => {
            let mut proc = ffmpeg_state.0.lock().await;
            proc.stop().await
        }
    }
}

#[tauri::command]
async fn save_replay_buffer(
    state: tauri::State<'_, AppStatusState>,
    obs_state: tauri::State<'_, ObsWsState>,
    ffmpeg_state: tauri::State<'_, FfmpegState>,
) -> Result<(), String> {
    let mode = { state.0.lock().unwrap().recording_mode.clone() };
    match mode {
        RecordingMode::Obs => {
            spawn(send_obs_command_wrapper(
                obs_state.0.clone(),
                "SaveReplayBuffer",
            )); // OBS に保存指示
            Ok(())
        }
        RecordingMode::Shell => {
            let mut proc = ffmpeg_state.0.lock().await;
            proc.save().await.map(|_| ())
        }
    }
}

#[tauri::command]
async fn get_saved_directory(state: tauri::State<'_, SettingsState>) -> Result<String, String> {
    // 現在設定されている保存先パスを返す
    Ok(state.0.lock().unwrap().save_dir.clone())
}

#[tauri::command]
async fn set_saved_directory(
    dir: String,
    settings: tauri::State<'_, SettingsState>,
    path: tauri::State<'_, SettingsPath>,
    obs_state: tauri::State<'_, ObsWsState>,
    ffmpeg_state: tauri::State<'_, FfmpegState>,
) -> Result<(), String> {
    {
        // 設定オブジェクトを更新して保存
        let mut set = settings.0.lock().unwrap();
        set.save_dir = dir.clone();
        save_settings(&path.0, &set).map_err(|e| e.to_string())?;
    }
    {
        // ffmpeg 側にも保存先を通知
        let mut proc = ffmpeg_state.0.lock().await;
        proc.set_save_dir(PathBuf::from(&dir));
    }
    obs::set_record_directory(obs_state.0.clone(), &dir).await
}

#[derive(Serialize)]
struct SavedVideoInfo {
    path: String,
    name: String,
    modified: String,
    size: u64,
}

#[tauri::command]
async fn list_saved_videos(
    settings: tauri::State<'_, SettingsState>,
) -> Result<Vec<SavedVideoInfo>, String> {
    // DB 内の孤立したレコードをクリーンアップ
    // cleanup_orphan_clips(&db.0).await?;

    // 現在の保存先ディレクトリを取得
    let save_dir = {
        let lock = settings.0.lock().unwrap();
        PathBuf::from(lock.save_dir.clone())
    };

    let mut files = Vec::new();
    if save_dir.exists() {
        // ディレクトリ内の mp4 ファイルを走査
        for entry in std::fs::read_dir(&save_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.extension().map(|ext| ext == "mp4").unwrap_or(false) {
                let meta = entry.metadata().map_err(|e| e.to_string())?;
                let modified: chrono::DateTime<chrono::Local> =
                    meta.modified().map_err(|e| e.to_string())?.into();
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                files.push(SavedVideoInfo {
                    path: path.to_string_lossy().to_string(),
                    name,
                    modified: modified.format("%Y-%m-%d %H:%M:%S").to_string(),
                    size: meta.len(),
                });
            }
        }
        // 更新日時で降順ソート
        files.sort_by(|a, b| b.modified.cmp(&a.modified));
    }

    Ok(files)
}

#[tauri::command]
async fn get_clip_metadata(
    path: String,
    db: tauri::State<'_, DbPath>,
) -> Result<Vec<db::EventWithOffset>, String> {
    get_clip_events(&db.0, std::path::Path::new(&path)).await
}

#[tauri::command]
fn load_settings_cmd(state: tauri::State<SettingsState>) -> AppSettings {
    state.0.lock().unwrap().clone()
}

#[tauri::command]
async fn save_settings_cmd(
    settings: AppSettings,
    state: tauri::State<'_, SettingsState>,
    path: tauri::State<'_, SettingsPath>,
    status: tauri::State<'_, AppStatusState>,
    ffmpeg: tauri::State<'_, FfmpegState>,
) -> Result<(), String> {
    {
        let mut lock = state.0.lock().unwrap();
        *lock = settings.clone();
    }
    {
        let mut st = status.0.lock().unwrap();
        st.recording_mode = settings.recording_mode.clone();
    }
    {
        let mut proc = ffmpeg.0.lock().await;
        proc.set_segment_seconds(settings.segment_seconds);
        proc.set_video_source(settings.video_source.clone());
        proc.set_audio_source(settings.audio_source.clone());
        proc.set_fps(settings.fps);
        proc.set_wrap_count(settings.wrap_count);
        proc.set_bitrate(settings.bitrate.clone());
        proc.set_save_dir(PathBuf::from(settings.save_dir.clone()));
    }
    save_settings(&path.0, &settings).map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_ffmpeg_replay(
    status_state: tauri::State<'_, AppStatusState>,
    state: tauri::State<'_, FfmpegState>,
    segment_seconds: Option<u32>,
    video_source: Option<String>,
    audio_source: Option<String>,
    fps: Option<u32>,
    wrap_count: Option<u32>,
    bitrate: Option<String>,
) -> Result<(), String> {
    info!(
        "Starting ffmpeg replay buffer with segment_seconds: {:?}, video_source: {:?}, audio_source: {:?}, fps: {:?}, wrap_count: {:?}, bitrate: {:?}",
        segment_seconds,
        video_source,
        audio_source,
        fps,
        wrap_count,
        bitrate
    );

    let mut proc = state.0.lock().await;
    {
        let mut status = status_state.0.lock().unwrap();
        status.replay_buffer_running = true;
    }
    if let Some(sec) = segment_seconds {
        proc.set_segment_seconds(sec);
    }
    if let Some(v) = video_source {
        proc.set_video_source(v);
    }
    if let Some(a) = audio_source {
        proc.set_audio_source(a);
    }
    if let Some(f) = fps {
        proc.set_fps(f);
    }

    let shared = state.0.clone();

    if let Some(w) = wrap_count {
        proc.set_wrap_count(w);
    }
    if let Some(b) = bitrate {
        proc.set_bitrate(b);
    }
    proc.start(shared).await
}

#[tauri::command]
async fn stop_ffmpeg_replay(
    status_state: tauri::State<'_, AppStatusState>,
    state: tauri::State<'_, FfmpegState>,
) -> Result<(), String> {
    let mut proc = state.0.lock().await;
    {
        let mut status = status_state.0.lock().unwrap();
        status.replay_buffer_running = false;
    }
    proc.stop().await
}

#[tauri::command]
async fn save_ffmpeg_clip(state: tauri::State<'_, FfmpegState>) -> Result<(), String> {
    let mut proc = state.0.lock().await;
    proc.save().await.map(|_| ())
}

#[derive(Serialize)]
struct DeviceInfo {
    index: i32,
    name: String,
}

#[derive(Serialize)]
struct DeviceList {
    video: Vec<DeviceInfo>,
    audio: Vec<DeviceInfo>,
}

#[tauri::command]
async fn list_ffmpeg_devices() -> Result<DeviceList, String> {
    let mut video = Vec::new();
    let mut audio = Vec::new();

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("ffmpeg")
            .stderr(Stdio::piped())
            .args(["-f", "avfoundation", "-list_devices", "true", "-i", ""])
            .output()
            .await
            .map_err(|e| e.to_string())?;
        if output.status.success() {
            // println!("ffmpeg command failed with status: {}", output.status);
            // println!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            // println!("stdout: {}", String::from_utf8_lossy(&output.stdout));

            return Err(String::from_utf8_lossy(&output.stderr).to_string());
        }

        let stderr_output = String::from_utf8_lossy(&output.stderr);
        let mut current: Option<&str> = None;
        for line in stderr_output.lines() {
            let trimmed = line.trim();
            if trimmed.contains("AVFoundation video devices") {
                current = Some("video");
                continue;
            }
            if trimmed.contains("AVFoundation audio devices") {
                current = Some("audio");
                continue;
            }
            if let Some(pos) = trimmed.find("] [") {
                if let Some(end) = trimmed[pos + 3..].find(']') {
                    let name = trimmed[pos + 3 + end + 1..].trim();
                    match current {
                        Some("video") => {
                            let index = video.len() as i32;
                            video.push(DeviceInfo {
                                index,
                                name: name.to_string(),
                            });
                        }
                        Some("audio") => {
                            let index = audio.len() as i32;
                            audio.push(DeviceInfo {
                                index,
                                name: name.to_string(),
                            });
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let audio_out = Command::new("arecord")
            .arg("-l")
            .output()
            .await
            .map_err(|e| e.to_string())?;
        if !audio_out.status.success() {
            return Err(String::from_utf8_lossy(&audio_out.stderr).to_string());
        }
        let video_out = Command::new("v4l2-ctl")
            .arg("--list-devices")
            .output()
            .await
            .map_err(|e| e.to_string())?;
        if !video_out.status.success() {
            return Err(String::from_utf8_lossy(&video_out.stderr).to_string());
        }

        for line in String::from_utf8_lossy(&audio_out.stdout).lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("card") {
                let index = audio.len() as i32;
                audio.push(DeviceInfo {
                    index,
                    name: trimmed.to_string(),
                });
            }
        }

        for line in String::from_utf8_lossy(&video_out.stdout).lines() {
            if line.trim().is_empty() {
                continue;
            }
            if !line.starts_with(' ') && !line.starts_with('\t') {
                let name = line.trim_end_matches(':').trim();
                let index = video.len() as i32;
                video.push(DeviceInfo {
                    index,
                    name: name.to_string(),
                });
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let output = Command::new("ffmpeg")
            .stderr(Stdio::piped())
            .args(["-list_devices", "true", "-f", "dshow", "-i", "dummy"])
            .output()
            .await
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).to_string());
        }

        let stderr_output = String::from_utf8_lossy(&output.stderr);
        let mut current: Option<&str> = None;
        for line in stderr_output.lines() {
            let trimmed = line.trim();
            if trimmed.contains("DirectShow video devices") {
                current = Some("video");
                continue;
            }
            if trimmed.contains("DirectShow audio devices") {
                current = Some("audio");
                continue;
            }
            if let Some(start) = trimmed.find('"') {
                if let Some(end) = trimmed[start + 1..].find('"') {
                    let name = &trimmed[start + 1..start + 1 + end];
                    match current {
                        Some("video") => {
                            let index = video.len() as i32;
                            video.push(DeviceInfo {
                                index,
                                name: name.to_string(),
                            });
                        }
                        Some("audio") => {
                            let index = audio.len() as i32;
                            audio.push(DeviceInfo {
                                index,
                                name: name.to_string(),
                            });
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        return Err("Unsupported OS".into());
    }

    Ok(DeviceList { video, audio })
}

/// アプリ状態を共有するためのラッパー
struct AppStatusState(Arc<Mutex<AppStatus>>);
/// 最新の LoL ゲームデータを保持するためのラッパー
struct LolDataState(Arc<Mutex<Option<AllGameData>>>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_logging();
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            get_status,
            set_recording_mode,
            start_recording,
            stop_recording,
            start_replay_buffer,
            stop_replay_buffer,
            save_replay_buffer,
            get_saved_directory,
            set_saved_directory,
            start_ffmpeg_replay,
            stop_ffmpeg_replay,
            save_ffmpeg_clip,
            list_ffmpeg_devices,
            list_saved_videos,
            get_clip_metadata,
            load_settings_cmd,
            save_settings_cmd,
            get_current_players,
            greet,
            show_notification,
            log_message])
        .setup( |_app| {

            let config_path = _app.path().config_dir().unwrap().join("settings.json");
            let settings = load_settings(&config_path).unwrap_or_default();
            let settings_for_state = settings.clone();
            let db_path = _app.path().app_local_data_dir().unwrap().join("clips.db");
            tauri::async_runtime::block_on(init_db(&db_path))?;
            tauri::async_runtime::block_on(cleanup_orphan_clips(&db_path))?;
            let db_state = DbPath(db_path.clone());
            let mut ffmpeg_path = PathBuf::new();
            ffmpeg_path.push("ffmpeg");
            let mut ffmpeg_proc = FfmpegProcess::new(ffmpeg_path);
            ffmpeg_proc.set_segment_seconds(settings.segment_seconds);
            ffmpeg_proc.set_video_source(settings.video_source.clone());
            ffmpeg_proc.set_audio_source(settings.audio_source.clone());
            ffmpeg_proc.set_fps(settings.fps);
            ffmpeg_proc.set_wrap_count(settings.wrap_count);
            ffmpeg_proc.set_bitrate(settings.bitrate.clone());
            ffmpeg_proc.set_save_dir(PathBuf::from(settings.save_dir.clone()));

            let status = AppStatus {
                game_state: GameState::NotStarted,
                obs_state: ObsState::Disconnected,
                recording_mode: settings.recording_mode.clone(),
                is_recording: false,
                replay_buffer_running: false,
            };
            let status = Arc::new(Mutex::new(status));

            let lol_data_state = LolDataState(Arc::new(Mutex::new(None)));

            let ffmpeg_process: SharedFfmpegProcess = Arc::new(AsyncMutex::new(ffmpeg_proc));
            let obs_ws_client: SharedObsWsClient = Arc::new(Mutex::new(None));
            let status_clone = status.clone();
            let settings_state = SettingsState(Arc::new(Mutex::new(settings_for_state)));
            let settings_state_clone2 = settings_state.clone();
            let settings_path_state = SettingsPath(config_path.clone());

            _app.manage(AppStatusState(status_clone.clone()));
            _app.manage(ObsWsState(obs_ws_client.clone()));
            _app.manage(FfmpegState(ffmpeg_process.clone()));
            _app.manage(settings_state);
            _app.manage(settings_path_state);
            _app.manage(db_state.clone());
            _app.manage(lol_data_state.clone());

            let dir = settings.save_dir.clone();
            let obs_ws_client_clone2 = obs_ws_client.clone();
            tauri::async_runtime::spawn(async move {
                let _ = set_record_directory(obs_ws_client_clone2, &dir).await;
            });

            let obs_ws_client_clone = obs_ws_client.clone();
            let ffmpeg_process_clone = ffmpeg_process.clone();
            let db_path_clone = db_state.clone();
            let lol_data_state_clone = lol_data_state.clone();

            tauri::async_runtime::spawn(async move {
                poll_lol_events(settings_state_clone2, move |all_data: &AllGameData, new_events: Vec<LolEvent>| {
                    {
                        let mut lock = lol_data_state_clone.0.lock().unwrap();
                        *lock = Some(all_data.clone());
                    }
                    let mut status = status_clone.lock().unwrap();
                    if status.game_state == GameState::NotStarted {
                        status.game_state = GameState::InProgress;
                    }
                    let mode = status.recording_mode.clone();
                    drop(status);

                    for event in new_events {
                        match event.EventName.as_str() {
                            "ChampionKill" => {
                                info!(
                                    "{} champion killed: {} by {}",
                                    event.EventTime,
                                    event.VictimName.as_deref().unwrap_or("Unknown"),
                                    event.KillerName.as_deref().unwrap_or("Unknown")
                                );
                            }
                            "Multikill" => {
                                if let Some(killer_name) = &event.KillerName {
                                    // アクティブプレイヤーの名前がキルしたプレイヤー名に含まれているか確認：うまく取れていない
                                    if !killer_name.contains(all_data.activePlayer.riotIdGameName.as_str()) {
                                        info!(
                                            "Killer name does not match active player: {} != {}",
                                            killer_name,
                                            all_data.activePlayer.riotIdGameName
                                        );
                                        continue;
                                    }

                                    info!(
                                        "{} multikill by {}: {}, active_player: {}",
                                        event.EventTime,
                                        killer_name,
                                        event.EventName,
                                        all_data.activePlayer.summonerName
                                    );
                                    info!(
                                        "Game Time: {}, Event Time:{}",
                                        all_data.gameData.gameTime,
                                        event.EventTime
                                    );

                                    match mode {
                                        RecordingMode::Obs => {
                                            let obs_client_clone = obs_ws_client_clone.clone();
                                            tauri::async_runtime::spawn(async move {
                                                if let Err(e) = send_obs_command_wrapper(obs_client_clone, "SaveReplayBuffer").await {
                                                    error!("Failed to send Multikill command to OBS: {}", e);
                                                } else {
                                                    info!("Sent Multikill command to OBS");
                                                }
                                            });
                                        }
                                        RecordingMode::Shell => {
                                            let ffmpeg_clone = ffmpeg_process_clone.clone();
                                            let event_clone = event.clone();
                                            let events_snapshot = all_data.events.events.clone();
                                            tauri::async_runtime::spawn({
                                                let db_path = db_path_clone.0.clone();
                                                async move {
                                                    let mut proc = ffmpeg_clone.lock().await;
                                                    match proc.save().await {
                                                        Ok(path) => {
                                                            let dur = proc.segment_seconds * 10;
                                                            let clip_start = event_clone.EventTime - dur as f64;
                                                            let relevant_events: Vec<LolEvent> = events_snapshot
                                                                .into_iter()
                                                                .filter(|ev| ev.EventTime >= clip_start)
                                                                .collect();
                                                            if let Err(e) = write_clip_metadata(&db_path, &path, &relevant_events, clip_start).await {
                                                                error!("Failed to write metadata: {}", e);
                                                            }
                                                        }
                                                        Err(e) => {
                                                            error!("Failed to save clip: {}", e);
                                                        }
                                                    }
                                                }
                                            });
                                        }
                                    }
                                }
                            }
                            
                            // ゲームスタート時にOBSかffmpegの録画を開始
                            "GameStart" => {
                                info!("Game started at {}", event.EventTime);

                                // ゲーム状態を更新
                                let mut status = status_clone.lock().unwrap();
                                status.game_state = GameState::InProgress;
                                status.obs_state = ObsState::Recording;
                                status.is_recording = true;
                                status.replay_buffer_running = false;

                                match mode {
                                    RecordingMode::Obs => {
                                        let obs_client_clone = obs_ws_client_clone.clone();
                                        tauri::async_runtime::spawn(async move {
                                            if let Err(e) = send_obs_command_wrapper(obs_client_clone, "StartRecord").await {
                                                error!("Failed to start OBS recording: {}", e);
                                            } else {
                                                info!("Started OBS recording");
                                            }
                                        });
                                    }
                                    RecordingMode::Shell => {
                                        let ffmpeg_clone = ffmpeg_process_clone.clone();
                                        tauri::async_runtime::spawn({
                                            let ffmpeg_process_clone = ffmpeg_process_clone.clone();
                                            async move {
                                                if let Err(e) = ffmpeg_clone.lock().await.start(ffmpeg_process_clone).await {
                                                    error!("Failed to start ffmpeg recording: {}", e);
                                                } else {
                                                    info!("Started ffmpeg recording");
                                                }
                                            }
                                        });
                                    }
                                }
                            }

                            // ゲーム終了時にOBSかffmpegの録画を停止
                            "GameEnd" => {
                                info!("Game ended at {}", event.EventTime);

                                // ゲーム状態を更新
                                let mut status = status_clone.lock().unwrap();
                                status.game_state = GameState::Finished;
                                status.obs_state = ObsState::NotRecording;
                                status.is_recording = false;
                                status.replay_buffer_running = false;
                                
                                match mode {
                                    RecordingMode::Obs => {
                                        let obs_client_clone = obs_ws_client_clone.clone();
                                        tauri::async_runtime::spawn(async move {
                                            if let Err(e) = send_obs_command_wrapper(obs_client_clone, "StopRecord").await {
                                                error!("Failed to stop OBS recording: {}", e);
                                            } else {
                                                info!("Stopped OBS recording");
                                            }
                                        });
                                    }
                                    RecordingMode::Shell => {
                                        let ffmpeg_clone = ffmpeg_process_clone.clone();
                                        tauri::async_runtime::spawn(async move {
                                            if let Err(e) = ffmpeg_clone.lock().await.stop().await {
                                                error!("Failed to stop ffmpeg recording: {}", e);
                                            } else {
                                                info!("Stopped ffmpeg recording");
                                            }
                                        });
                                    }
                                }
                            }
                            _ => {
                                info!("Unhandled event: {} at {}", event.EventName, event.EventTime);
                            }
                        }
                    }
                }).await;
            });

            // アプリ起動時に通知を表示
            _app.notification()
                .builder()
                .title("LOL Replay")
                .body("LOL Replay is running")
                .show()
                .unwrap();

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn poll_lol_events<F>(settings: SettingsState, mut callback: F)
where
    F: FnMut(&AllGameData, Vec<LolEvent>) + Send + 'static,
{
    // LoL クライアントのイベント API を定期的にポーリング
    let client = Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap();
    let mut seen_event_ids: HashSet<i64> = HashSet::new();
    let mut pending_events: Vec<(LolEvent, f64)> = Vec::new();

    info!("Starting LoL event polling...");

    loop {
        let delay_secs = {
            let lock = settings.0.lock().unwrap();
            lock.event_trigger_delay
        };
        match client
            .get("https://127.0.0.1:2999/liveclientdata/allgamedata")
            .send()
            .await
        {
            Ok(response) => {
                if let Ok(body) = response.text().await {
                    match serde_json::from_str::<AllGameData>(&body) {
                        Ok(all_data) => {
                            let current_time = all_data.gameData.gameTime;

                            for event in all_data.clone().events.events.into_iter() {
                                if !seen_event_ids.contains(&event.EventID) {
                                    seen_event_ids.insert(event.EventID.clone());
                                    pending_events.push((event.clone(), event.EventTime.clone()));
                                }
                            }

                            let mut ready_events = Vec::new();
                            pending_events.retain(|(ev, detect_time)| {
                                if current_time - *detect_time >= delay_secs {
                                    ready_events.push(ev.clone());
                                    false
                                } else {
                                    true
                                }
                            });

                            if !ready_events.is_empty() {
                                info!("Delayed events triggered: {}", ready_events.len());
                                callback(&all_data, ready_events);
                            }

                        }
                        Err(e) => {
                            error!("Failed to parse AllGameData from response: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                // eprintln!("Error fetching LoL events: {}", e);
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

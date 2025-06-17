use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;
use std::time::Duration;
use std::collections::HashSet;
use tokio::process::Command;
use std::process::Stdio;
use reqwest::Client;
use tauri::{async_runtime::spawn, Manager};
use std::path::PathBuf;

mod settings;
mod lol;
mod obs;
mod ffmpeg;
mod db;

use settings::{load_settings, save_settings, AppSettings, SettingsPath, SettingsState};
use lol::{AllGameData, LolEvent};
use obs::{send_obs_command_wrapper, set_record_directory, ObsWsState, SharedObsWsClient};
use ffmpeg::{FfmpegProcess, FfmpegState, SharedFfmpegProcess, write_clip_metadata};
use db::{DbPath, init_db, get_clip_events, list_clips, cleanup_orphan_clips};

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
struct AppStatus {
    game_state: GameState,
    obs_state: ObsState,
    recording_mode: RecordingMode,
    is_recording: bool,
    replay_buffer_running: bool,
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn get_status(state: tauri::State<AppStatusState>) -> AppStatus {
    state.0.lock().unwrap().clone()
}

#[tauri::command]
async fn set_recording_mode(state: tauri::State<'_, AppStatusState>, mode: RecordingMode) -> Result<(), String> {
    let mut lock = state.0.lock().unwrap();
    lock.recording_mode = mode;

    println!("Recording mode set to: {:?}", lock.recording_mode);
    Ok(())
}

#[tauri::command]
async fn start_recording(
    state: tauri::State<'_, AppStatusState>,
    obs_state: tauri::State<'_, ObsWsState>,
    ffmpeg_state: tauri::State<'_, FfmpegState>,
) -> Result<(), String> {
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
            spawn(send_obs_command_wrapper(obs_state.0.clone(), "StartRecord"));
            Ok(())
        }
        RecordingMode::Shell => {
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
            spawn(send_obs_command_wrapper(obs_state.0.clone(), "StopRecord"));
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
    let mode = {
        let mut lock = state.0.lock().unwrap();
        lock.is_recording = true;
        lock.replay_buffer_running = true;
        lock.recording_mode.clone()
    };
    match mode {
        RecordingMode::Obs => {
            spawn(send_obs_command_wrapper(obs_state.0.clone(), "StartReplayBuffer"));
            Ok(())
        }
        RecordingMode::Shell => {
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
    let mode = {
        let mut lock = state.0.lock().unwrap();
        lock.is_recording = false;
        lock.replay_buffer_running = false;
        lock.recording_mode.clone()
    };
    match mode {
        RecordingMode::Obs => {
            spawn(send_obs_command_wrapper(obs_state.0.clone(), "StopReplayBuffer"));
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
            spawn(send_obs_command_wrapper(obs_state.0.clone(), "SaveReplayBuffer"));
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
        let mut set = settings.0.lock().unwrap();
        set.save_dir = dir.clone();
        save_settings(&path.0, &set).map_err(|e| e.to_string())?;
    }
    {
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
async fn list_saved_videos(db: tauri::State<'_, DbPath>) -> Result<Vec<SavedVideoInfo>, String> {
    cleanup_orphan_clips(&db.0).await?;
    let clips = list_clips(&db.0).await?;
    let mut files = Vec::new();
    for (path, created, size) in clips {
        let name = std::path::Path::new(&path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        files.push(SavedVideoInfo {
            path,
            name,
            modified: created,
            size,
        });
    }
    Ok(files)
}

#[tauri::command]
async fn get_clip_metadata(path: String, db: tauri::State<'_, DbPath>) -> Result<Vec<db::EventWithOffset>, String> {
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
            .args([
                "-f",
                "avfoundation",
                "-list_devices",
                "true",
                "-i",
                "",
            ])
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
                            video.push(DeviceInfo { index, name: name.to_string() });
                        }
                        Some("audio") => {
                            let index = audio.len() as i32;
                            audio.push(DeviceInfo { index, name: name.to_string() });
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
                audio.push(DeviceInfo { index, name: trimmed.to_string() });
            }
        }

        for line in String::from_utf8_lossy(&video_out.stdout).lines() {
            if line.trim().is_empty() {
                continue;
            }
            if !line.starts_with(' ') && !line.starts_with('\t') {
                let name = line.trim_end_matches(':').trim();
                let index = video.len() as i32;
                video.push(DeviceInfo { index, name: name.to_string() });
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
                            video.push(DeviceInfo { index, name: name.to_string() });
                        }
                        Some("audio") => {
                            let index = audio.len() as i32;
                            audio.push(DeviceInfo { index, name: name.to_string() });
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

struct AppStatusState(Arc<Mutex<AppStatus>>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
            greet])
        .setup(move |_app| {

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

            let ffmpeg_process: SharedFfmpegProcess = Arc::new(AsyncMutex::new(ffmpeg_proc));
            let obs_ws_client: SharedObsWsClient = Arc::new(Mutex::new(None));
            let status_clone = status.clone();
            let settings_state = SettingsState(Arc::new(Mutex::new(settings_for_state)));
            let settings_path_state = SettingsPath(config_path.clone());

            _app.manage(AppStatusState(status_clone.clone()));
            _app.manage(ObsWsState(obs_ws_client.clone()));
            _app.manage(FfmpegState(ffmpeg_process.clone()));
            _app.manage(settings_state);
            _app.manage(settings_path_state);
            _app.manage(db_state.clone());

            let dir = settings.save_dir.clone();
            let obs_ws_client_clone2 = obs_ws_client.clone();
            tauri::async_runtime::spawn(async move {
                let _ = set_record_directory(obs_ws_client_clone2, &dir).await;
            });

            let obs_ws_client_clone = obs_ws_client.clone();
            let ffmpeg_process_clone = ffmpeg_process.clone();
            let db_path_clone = db_state.clone();

            tauri::async_runtime::spawn(async move {
                poll_lol_events(move |all_data: &AllGameData, new_events: Vec<LolEvent>| {
                    let mut status = status_clone.lock().unwrap();
                    if status.game_state == GameState::NotStarted {
                        status.game_state = GameState::InProgress;
                    }
                    let mode = status.recording_mode.clone();
                    drop(status);

                    for event in new_events {
                        match event.EventName.as_str() {
                            "ChampionKill" => {
                                println!("{} champion killed: {} by {}", event.EventTime, event.VictimName.as_deref().unwrap_or("Unknown"), event.KillerName.as_deref().unwrap_or("Unknown"));
                            }
                            "Multikill" => {
                                if let Some(killer_name) = &event.KillerName {
                                    if !killer_name.contains(all_data.activePlayer.summonerName.as_str()) {
                                        continue;
                                    }
                                    match mode {
                                        RecordingMode::Obs => {
                                            let obs_client_clone = obs_ws_client_clone.clone();
                                            tauri::async_runtime::spawn(async move {
                                                if let Err(e) = send_obs_command_wrapper(obs_client_clone, "SaveReplayBuffer").await {
                                                    eprintln!("Failed to send Multikill command to OBS: {}", e);
                                                } else {
                                                    println!("Sent Multikill command to OBS");
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
                                                                eprintln!("Failed to write metadata: {}", e);
                                                            }
                                                        }
                                                        Err(e) => {
                                                            eprintln!("Failed to save clip: {}", e);
                                                        }
                                                    }
                                                }
                                            });
                                        }
                                    }
                                }
                            }
                            _ => {
                                println!("Unhandled event: {} at {}", event.EventName, event.EventTime);
                            }
                        }
                    }
                }).await;
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn poll_lol_events<F>(mut callback: F)
where
    F: FnMut(&AllGameData, Vec<LolEvent>) + Send + 'static,
{
    let client = Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap();
    let mut last_event_ids: HashSet<i64> = HashSet::new();

    println!("Starting LoL event polling...");

    loop {
        match client.get("https://127.0.0.1:2999/liveclientdata/allgamedata").send().await {
            Ok(response) => {
                if let Ok(body) = response.text().await {
                    match serde_json::from_str::<AllGameData>(&body) {
                        Ok(all_data) => {
                            let new_events: Vec<LolEvent> = all_data.clone().events.events.into_iter()
                                .filter(|event| !last_event_ids.contains(&event.EventID))
                                .collect();
                            if !new_events.is_empty() {
                                println!("New events detected: {}", new_events.len());
                                callback(&all_data, new_events.clone());
                                for event in &new_events {
                                    last_event_ids.insert(event.EventID);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to parse AllGameData from response: {}", e);
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


use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;
use std::time::Duration;
use std::collections::HashSet;
use std::path::PathBuf;
use tokio::process::Command;
use tokio::fs;
use reqwest::Client;
use tauri::async_runtime::spawn;

mod settings;
mod lol;
mod obs;
mod ffmpeg;

use settings::{load_settings, save_settings, AppSettings, SettingsPath, SettingsState};
use lol::{AllGameData, LolEvent};
use obs::{send_obs_command_wrapper, send_obs_request_wrapper, ObsWsState, SharedObsWsClient};
use ffmpeg::{FfmpegProcess, FfmpegState, SharedFfmpegProcess, ensure_ffmpeg_path, write_clip_metadata};

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
            let mut proc = ffmpeg_state.0.lock().await;
            proc.start().await
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
            let mut proc = ffmpeg_state.0.lock().await;
            proc.start().await
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
async fn get_saved_directory(_state: tauri::State<'_, AppStatusState>, obs_state: tauri::State<'_, ObsWsState>) -> Result<String, String> {
    let resp = send_obs_request_wrapper(obs_state.0.clone(), "GetRecordDirectory").await?;
    let dir = resp
        .get("d")
        .and_then(|d| d.get("responseData"))
        .and_then(|rd| rd.get("recordDirectory"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Ok(dir)
}

#[derive(Serialize)]
struct SavedVideoInfo {
    path: String,
    name: String,
    modified: String,
    size: u64,
}

#[tauri::command]
async fn list_saved_videos(obs_state: tauri::State<'_, ObsWsState>) -> Result<Vec<SavedVideoInfo>, String> {
    let resp = send_obs_request_wrapper(obs_state.0.clone(), "GetRecordDirectory").await?;
    let dir = resp
        .get("d")
        .and_then(|d| d.get("responseData"))
        .and_then(|rd| rd.get("recordDirectory"))
        .and_then(|v| v.as_str())
        .ok_or("no dir")?;
    let mut entries = fs::read_dir(dir).await.map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    while let Some(ent) = entries.next_entry().await.map_err(|e| e.to_string())? {
        let path = ent.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "mp4" || ext == "mkv" || ext == "mov" {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let (modified, size) = match ent.metadata().await {
                        Ok(meta) => {
                            let modified = meta
                                .modified()
                                .ok()
                                .map(|t| {
                                    let dt: chrono::DateTime<chrono::Local> = t.into();
                                    dt.format("%Y-%m-%d %H:%M:%S").to_string()
                                })
                                .unwrap_or_default();
                            (modified, meta.len())
                        }
                        Err(_) => (String::new(), 0),
                    };
                    files.push(SavedVideoInfo {
                        path: path.to_string_lossy().to_string(),
                        name,
                        modified,
                        size,
                    });
                }
            }
        }
    }
    Ok(files)
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
    proc.start().await
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
async fn list_ffmpeg_devices(state: tauri::State<'_, FfmpegState>) -> Result<DeviceList, String> {
    let ffmpeg_path = {
        let proc = state.0.lock().await;
        proc.ffmpeg_path.clone()
    };
    let output = Command::new(ffmpeg_path)
        .arg("-f")
        .arg("avfoundation")
        .arg("-list_devices")
        .arg("true")
        .arg("-i")
        .arg("")
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut video = Vec::new();
    let mut audio = Vec::new();
    let mut current = None::<&str>;

    for line in stderr.lines() {
        if line.contains("AVFoundation video devices") {
            current = Some("video");
            continue;
        }
        if line.contains("AVFoundation audio devices") {
            current = Some("audio");
            continue;
        }
        if let Some(kind) = current {
            let trimmed = line.trim();
            if let Some(start) = trimmed.find('[') {
                if let Some(end) = trimmed[start + 1..].find(']') {
                    let idx_str = &trimmed[start + 1..start + 1 + end];
                    if let Ok(index) = idx_str.parse::<i32>() {
                        let name = trimmed[start + 1 + end + 1..].trim();
                        let info = DeviceInfo { index, name: name.to_string() };
                        match kind {
                            "video" => video.push(info),
                            "audio" => audio.push(info),
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    Ok(DeviceList { video, audio })
}

struct AppStatusState(Arc<Mutex<AppStatus>>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let ctx = tauri::generate_context!();
    let config_path = tauri::api::path::app_config_dir(&ctx.config())
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("settings.json");
    let settings = load_settings(&config_path).unwrap_or_default();

    let ffmpeg_path = tauri::async_runtime::block_on(ensure_ffmpeg_path(&ctx.config())).expect("ffmpeg setup");

    let status = Arc::new(Mutex::new(AppStatus {
        game_state: GameState::NotStarted,
        obs_state: ObsState::Disconnected,
        recording_mode: settings.recording_mode.clone(),
        is_recording: false,
        replay_buffer_running: false,
    }));

    let obs_ws_client: SharedObsWsClient = Arc::new(Mutex::new(None));
    let mut ffmpeg_proc = FfmpegProcess::new(ffmpeg_path);
    ffmpeg_proc.set_segment_seconds(settings.segment_seconds);
    ffmpeg_proc.set_video_source(settings.video_source.clone());
    ffmpeg_proc.set_audio_source(settings.audio_source.clone());
    ffmpeg_proc.set_fps(settings.fps);
    let ffmpeg_process: SharedFfmpegProcess = Arc::new(AsyncMutex::new(ffmpeg_proc));
    let settings_state = SettingsState(Arc::new(Mutex::new(settings)));
    let settings_path_state = SettingsPath(config_path.clone());

    let status_clone = status.clone();

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
            start_ffmpeg_replay,
            stop_ffmpeg_replay,
            save_ffmpeg_clip,
            list_ffmpeg_devices,
            list_saved_videos,
            load_settings_cmd,
            save_settings_cmd,
            greet])
        .manage(AppStatusState(status.clone()))
        .manage(ObsWsState(obs_ws_client.clone()))
        .manage(FfmpegState(ffmpeg_process.clone()))
        .manage(settings_state)
        .manage(settings_path_state)
        .setup(move |_app| {
            let obs_ws_client_clone = obs_ws_client.clone();
            let ffmpeg_process_clone = ffmpeg_process.clone();

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
                                            tauri::async_runtime::spawn(async move {
                                                let mut proc = ffmpeg_clone.lock().await;
                                                match proc.save().await {
                                                    Ok(path) => {
                                                        let dur = proc.segment_seconds * 10;
                                                        let clip_start = event_clone.EventTime - dur as f64;
                                                        let relevant_events: Vec<LolEvent> = events_snapshot
                                                            .into_iter()
                                                            .filter(|ev| ev.EventTime >= clip_start)
                                                            .collect();
                                                        if let Err(e) = write_clip_metadata(&path, &relevant_events, clip_start).await {
                                                            eprintln!("Failed to write metadata: {}", e);
                                                        }
                                                    }
                                                    Err(e) => {
                                                        eprintln!("Failed to save clip: {}", e);
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
        .run(ctx)
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
                    if let Ok(all_data) = serde_json::from_str::<AllGameData>(&body){
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
                    } else {
                        eprintln!("Failed to parse AllGameData from response:");
                    }
                }
            }
            Err(e) => {
                eprintln!("Error fetching LoL events: {}", e);
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}


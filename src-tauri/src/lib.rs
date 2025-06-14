// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;
use std::time::Duration;
use serde_json::json;
use tauri::async_runtime::spawn;
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use std::process::Stdio;
use nix::sys::signal::{kill, Signal::SIGTERM};
use nix::unistd::Pid;
use tokio::io::AsyncWriteExt;
use reqwest::Client;
use std::collections::HashSet;
use std::path::PathBuf;
use tokio::fs;

mod settings;
use settings::{load_settings, save_settings, AppSettings, SettingsPath, SettingsState};

type WsType = WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>;

async fn ensure_ffmpeg_path(config: &tauri::Config) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut dir = tauri::api::path::app_local_data_dir(config)
        .ok_or("no data dir")?;
    dir.push("ffmpeg");
    fs::create_dir_all(&dir).await?;
    let bin_name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let bin_path = dir.join(bin_name);
    if bin_path.exists() {
        return Ok(bin_path);
    }

    let url = if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffmpeg-mac-arm64"
    } else if cfg!(target_os = "macos") {
        "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffmpeg-mac-x64"
    } else if cfg!(target_os = "windows") {
        "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffmpeg-win32-x64.exe"
    } else {
        "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffmpeg-linux-x64"
    };

    let bytes = reqwest::get(url).await?.bytes().await?;
    fs::write(&bin_path, &bytes).await?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perm = fs::metadata(&bin_path).await?.permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&bin_path, perm).await?;
    }

    Ok(bin_path)
}

pub struct ObsWsClient {
    ws: WsType,
}

impl ObsWsClient {
    /// 接続＋Identifyまでセットアップ
    pub async fn connect_and_identify(url: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let (mut ws_stream, _) = connect_async(url).await?;
        // 1. Hello
        let msg = ws_stream.next().await;
        if let Some(Ok(Message::Text(text))) = msg {
            let val: serde_json::Value = serde_json::from_str(&text)?;
            if val.get("op").and_then(|v| v.as_u64()) != Some(0) {
                return Err("No Hello from OBS".into());
            }
        } else {
            return Err("Failed to receive Hello".into());
        }
        // 2. Identify
        let identify = json!({
            "op": 1,
            "d": { "rpcVersion": 1, "authentication": null }
        });
        ws_stream.send(Message::Text(identify.to_string().into())).await?;
        // 3. Identified
        let msg = ws_stream.next().await;
        if let Some(Ok(Message::Text(text))) = msg {
            let val: serde_json::Value = serde_json::from_str(&text)?;
            if val.get("op").and_then(|v| v.as_u64()) != Some(2) {
                return Err("No Identified from OBS".into());
            }
        } else {
            return Err("Failed to receive Identified".into());
        }
        Ok(ObsWsClient { ws: ws_stream })
    }

    /// 任意のコマンド送信
    pub async fn send_command(&mut self, command: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let req = json!({
            "op": 6,
            "d": {
                "requestType": command,
                "requestId": "tauri-lol-obs-001"
            }
        });
        self.ws.send(Message::Text(req.to_string().into())).await?;
        Ok(())
    }

    /// 任意のコマンド送信してレスポンスを受け取る
    pub async fn send_request(&mut self, command: &str) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let req = json!({
            "op": 6,
            "d": {
                "requestType": command,
                "requestId": "tauri-lol-obs-001"
            }
        });
        self.ws.send(Message::Text(req.to_string().into())).await?;
        if let Some(Ok(Message::Text(resp))) = self.ws.next().await {
            let val: serde_json::Value = serde_json::from_str(&resp)?;
            Ok(val)
        } else {
            Err("No response".into())
        }
    }
}


#[derive(Debug, Deserialize, Clone)]
pub struct AllGameData {
    pub activePlayer: ActivePlayer,
    pub allPlayers: Vec<Player>,
    pub events: EventData,
    pub gameData: GameData,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ActivePlayer {
    pub abilities: Abilities,
    pub championStats: ChampionStats,
    pub currentGold: f64,
    pub fullRunes: FullRunes,
    pub level: u32,
    pub riotId: String,
    pub riotIdGameName: String,
    pub riotIdTagLine: String,
    pub summonerName: String,
    pub teamRelativeColors: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Abilities {
    #[serde(rename = "Q")]
    pub q: Option<Ability>,
    #[serde(rename = "W")]
    pub w: Option<Ability>,
    #[serde(rename = "E")]
    pub e: Option<Ability>,
    #[serde(rename = "R")]
    pub r: Option<Ability>,
    #[serde(rename = "Passive")]
    pub passive: Option<Ability>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Ability {
    #[serde(default)]
    pub abilityLevel: Option<u32>,
    pub displayName: String,
    pub id: String,
    pub rawDescription: String,
    pub rawDisplayName: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChampionStats {
    pub abilityHaste: f64,
    pub abilityPower: f64,
    pub armor: f64,
    pub armorPenetrationFlat: f64,
    pub armorPenetrationPercent: f64,
    pub attackDamage: f64,
    pub attackRange: f64,
    pub attackSpeed: f64,
    pub bonusArmorPenetrationPercent: f64,
    pub bonusMagicPenetrationPercent: f64,
    pub critChance: f64,
    pub critDamage: f64,
    pub currentHealth: f64,
    pub healShieldPower: f64,
    pub healthRegenRate: f64,
    pub lifeSteal: f64,
    pub magicLethality: f64,
    pub magicPenetrationFlat: f64,
    pub magicPenetrationPercent: f64,
    pub magicResist: f64,
    pub maxHealth: f64,
    pub moveSpeed: f64,
    pub omnivamp: f64,
    pub physicalLethality: f64,
    pub physicalVamp: f64,
    pub resourceMax: f64,
    pub resourceRegenRate: f64,
    pub resourceType: String,
    pub resourceValue: f64,
    pub spellVamp: f64,
    pub tenacity: f64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FullRunes {
    pub generalRunes: Vec<Rune>,
    pub keystone: Rune,
    pub primaryRuneTree: RuneTree,
    pub secondaryRuneTree: RuneTree,
    pub statRunes: Vec<StatRune>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Rune {
    pub displayName: String,
    pub id: u32,
    pub rawDescription: String,
    pub rawDisplayName: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RuneTree {
    pub displayName: String,
    pub id: u32,
    pub rawDescription: String,
    pub rawDisplayName: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StatRune {
    pub id: u32,
    pub rawDescription: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Player {
    pub championName: String,
    pub isBot: bool,
    pub isDead: bool,
    pub items: Vec<Item>,
    pub level: u32,
    pub position: String,
    pub rawChampionName: String,
    pub rawSkinName: String,
    pub respawnTimer: f64,
    pub riotId: String,
    pub riotIdGameName: String,
    pub riotIdTagLine: String,
    pub runes: PlayerRunes,
    pub scores: Scores,
    pub skinID: i32,
    pub skinName: String,
    pub summonerName: String,
    pub summonerSpells: SummonerSpells,
    pub team: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Item {
    pub canUse: bool,
    pub consumable: bool,
    pub count: u32,
    pub displayName: String,
    pub itemID: i32,
    pub price: u32,
    pub rawDescription: String,
    pub rawDisplayName: String,
    pub slot: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PlayerRunes {
    pub keystone: Rune,
    pub primaryRuneTree: RuneTree,
    pub secondaryRuneTree: RuneTree,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Scores {
    pub assists: u32,
    pub creepScore: u32,
    pub deaths: u32,
    pub kills: u32,
    pub wardScore: f64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SummonerSpells {
    pub summonerSpellOne: SummonerSpell,
    pub summonerSpellTwo: SummonerSpell,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SummonerSpell {
    pub displayName: String,
    pub rawDescription: String,
    pub rawDisplayName: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct EventData {
    #[serde(rename = "Events")]
    pub events: Vec<LolEvent>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LolEvent {
    pub EventID: i64,
    pub EventName: String,
    pub EventTime: f64,

    // オプションなフィールドが多い！
    pub Assisters: Option<Vec<String>>,
    pub KillerName: Option<String>,
    pub VictimName: Option<String>,
    pub KillStreak: Option<u32>,
    pub Recipient: Option<String>,
    pub Result: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GameData {
    pub gameMode: String,
    pub gameTime: f64,
    pub mapName: String,
    pub mapNumber: i32,
    pub mapTerrain: String,
}





// OBS WebSocketの状態

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
enum RecordingMode {
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
            proc.save().await
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
    proc.save().await
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


type SharedObsWsClient = Arc<Mutex<Option<ObsWsClient>>>;
struct ObsWsState(SharedObsWsClient);


struct FfmpegProcess {
    child: Option<Child>,
    ffmpeg_path: PathBuf,
    segment_seconds: u32,
    video_source: String,
    audio_source: String,
    fps: u32,
}

impl FfmpegProcess {
    fn new(ffmpeg_path: PathBuf) -> Self {
        Self {
            child: None,
            ffmpeg_path,
            segment_seconds: 6,
            video_source: "1".into(),
            audio_source: "none".into(),
            fps: 30,
        }
    }

    fn set_segment_seconds(&mut self, secs: u32) {
        self.segment_seconds = secs;
    }

    fn set_video_source(&mut self, src: String) {
        self.video_source = src;
    }

    fn set_ffmpeg_path(&mut self, path: PathBuf) {
        self.ffmpeg_path = path;
    }

    fn set_audio_source(&mut self, src: String) {
        self.audio_source = src;
    }

    fn set_fps(&mut self, fps: u32) {
        self.fps = fps;
    }

    async fn start(&mut self) -> Result<(), String> {
        if self.child.is_some() {
            return Ok(());
        }
        let child = Command::new("sh")
            .arg("./ffmpeg_replaybuffer.sh")
            .env("FFMPEG_BIN", &self.ffmpeg_path)
            .arg("-t")
            .arg(self.segment_seconds.to_string())
            .arg("-f")
            .arg(self.fps.to_string())
            .arg("-s")
            .arg(format!("{}:{}", self.video_source, self.audio_source))
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        self.child = Some(child);
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), String> {
        if let Some(mut child) = self.child.take() {
            let mut sent = false;
            if let Some(stdin) = child.stdin.as_mut() {
                if stdin.write_all(b"q").await.is_ok() {
                    sent = true;
                }
            }
            if !sent {
                if let Some(id) = child.id() {
                    kill(Pid::from_raw(id as i32), SIGTERM).map_err(|e| e.to_string())?;
                }
            }
            let _ = child.wait().await;
        }
        Ok(())
    }

    async fn save(&mut self) -> Result<(), String> {
        if let Some(child) = &mut self.child {
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(b"s").await.map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}

type SharedFfmpegProcess = Arc<AsyncMutex<FfmpegProcess>>;
struct FfmpegState(SharedFfmpegProcess);

// 初回コマンド送信時
async fn send_obs_command_wrapper(shared: SharedObsWsClient, command: &str) -> Result<(), String> {
    let mut needs_connect = false;
    {
        let client_guard = shared.lock().unwrap();
        needs_connect = client_guard.is_none();
    }
    if needs_connect {
        let client = ObsWsClient::connect_and_identify("ws://127.0.0.1:4455")
            .await
            .map_err(|e| e.to_string())?;
        let mut client_guard = shared.lock().unwrap();
        *client_guard = Some(client);
    }

    // ここからが重要！
    // Mutexをまたいだまま await しないために、一旦値を取り出して drop する
    let mut client_opt = {
        let mut guard = shared.lock().unwrap();
        guard.take()
    };
    let result = if let Some(ref mut client) = client_opt {
        client.send_command(command).await.map_err(|e| e.to_string())
    } else {
        Err("OBS WS Client not connected".into())
    };

    // 終わったら Mutex に値を戻す
    {
        let mut guard = shared.lock().unwrap();
        *guard = client_opt;
    }
    result
}

// コマンド送信してレスポンスを受け取る
async fn send_obs_request_wrapper(shared: SharedObsWsClient, command: &str) -> Result<serde_json::Value, String> {
    let mut needs_connect = false;
    {
        let client_guard = shared.lock().unwrap();
        needs_connect = client_guard.is_none();
    }
    if needs_connect {
        let client = ObsWsClient::connect_and_identify("ws://127.0.0.1:4455")
            .await
            .map_err(|e| e.to_string())?;
        let mut client_guard = shared.lock().unwrap();
        *client_guard = Some(client);
    }

    let mut client_opt = {
        let mut guard = shared.lock().unwrap();
        guard.take()
    };
    let result = if let Some(ref mut client) = client_opt {
        client.send_request(command).await.map_err(|e| e.to_string())
    } else {
        Err("OBS WS Client not connected".into())
    };
    {
        let mut guard = shared.lock().unwrap();
        *guard = client_opt;
    }
    result
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

    // グローバルで
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
            load_settings_cmd,
            save_settings_cmd,
            greet])
        .manage(AppStatusState(status.clone()))
        .manage(ObsWsState(obs_ws_client.clone()))
        .manage(FfmpegState(ffmpeg_process.clone()))
        .manage(settings_state)
        .manage(settings_path_state)
        .setup(move |_app| {

            // OBS WebSocketクライアントの初期化
            let obs_ws_client_clone = obs_ws_client.clone();
            let ffmpeg_process_clone = ffmpeg_process.clone();

            // LoLのイベントをポーリングして処理するスレッド
            tauri::async_runtime::spawn(async move {
                // LoLのイベントをポーリング
                poll_lol_events(move |all_data: &AllGameData, new_events: Vec<LolEvent>| {
                    // ゲーム状態の更新
                    let mut status = status_clone.lock().unwrap();
                    if status.game_state == GameState::NotStarted {
                        status.game_state = GameState::InProgress;
                    }
                    let mode = status.recording_mode.clone();
                    drop(status);
                    
                    // 現在操作中のプレイヤー情報をログに出力
                    // println!("Active Player: {} ({})", all_data.active_player.summoner_name, all_data.active_player.champion_name);

                    // 新しいイベントを処理
                    for event in new_events {
                        match event.EventName.as_str() {
                            "ChampionKill" => {
                                println!("{} champion killed: {} by {}", event.EventTime, event.VictimName.as_deref().unwrap_or("Unknown"), event.KillerName.as_deref().unwrap_or("Unknown"));
                            }
                            "Multikill" => {
                                // active_player == killer_name の場合、OBSに送信
                                if let Some(killer_name) = &event.KillerName {
                                    if !killer_name.contains(all_data.activePlayer.summonerName.as_str()) {
                                        continue; // multikills by other players are ignored
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
                                            tauri::async_runtime::spawn(async move {
                                                let mut proc = ffmpeg_clone.lock().await;
                                                if let Err(e) = proc.save().await {
                                                    eprintln!("Failed to save clip: {}", e);
                                                }
                                            });
                                        }
                                    }
                                }
                            
                            }
                            "TurretKilled" => {
                                
                            }
                            "DragonKill" => {
                                
                            }
                            _ => {
                                // 他のイベントは無視
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

/// LoLのイベントをポーリングしてコールバックを呼び出す
async fn poll_lol_events<F>(mut callback: F)
where
    F: FnMut(&AllGameData, Vec<LolEvent>) + Send + 'static,
{
    let client = Client::builder()
        .danger_accept_invalid_certs(true) // LoLのローカルAPIは自己署名証明書
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
                        // 新しいイベントのみを処理
                        let new_events: Vec<LolEvent> = all_data.clone().events.events.into_iter()
                            .filter(|event| !last_event_ids.contains(&event.EventID))
                            .collect();

                        if !new_events.is_empty() {
                            // コールバックを呼び出す
                            println!("New events detected: {}", new_events.len());
                            callback(&all_data, new_events.clone());

                            // 新しいイベントIDを記録
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


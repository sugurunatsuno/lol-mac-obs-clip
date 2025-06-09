// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use serde_json::json;
use std::error::Error;
use futures_util::SinkExt;

// 非同期関数
pub async fn obs_send_command(command: &str) -> Result<(), Box<dyn Error>> {
    let url = "ws://127.0.0.1:4455";
    let (mut ws_stream, _) = connect_async(url).await?;

    let req = json!({
        "op": 6,
        "d": {
            "requestType": command,
            "requestId": "tauri-lol-obs-001"
        }
    });

    // ws_stream.send(Message::Text(req.to_string().into())).await?;
    // 必要ならレスポンスも受信可
    Ok(())
}

/// 全体のゲームデータを保持する構造体
#[derive(Deserialize)]
pub struct AllGameData {
    pub gameData: GameData,
    pub events: EventData,
    pub allPlayers: Vec<Player>,
  // 必要に応じて追加
}

/// ゲームの状態を保持する構造体
#[derive(Deserialize)]
pub struct GameData {
  pub gameTime: f64,
  pub gameMode: String,
  pub mapName: String,
}

/// イベントデータを保持する構造体
#[derive(Deserialize)]
pub struct EventData {
  pub Events: Vec<LolEvent>,
}

/// LoLのイベントデータを保持する構造体
#[derive(Deserialize)]
pub struct LolEvent {
  pub EventID: i64,
  pub EventName: String,
  pub EventTime: f64,
}

/// プレイヤーの情報を保持する構造体
#[derive(Deserialize)]
pub struct Player {
  pub summonerName: String,
  pub championName: String,
  pub team: String,
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

#[derive(Debug, Clone, Serialize)]
struct AppStatus {
    game_state: GameState,
    obs_state: ObsState,
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
fn start_recording(state: tauri::State<AppStatusState>) {
    let mut lock = state.0.lock().unwrap();
    // 本来はobs_start_recording()を非同期で実装推奨！
    let _ = obs_start_recording();
    let _ = obs_start_replay_buffer();
    lock.game_state = GameState::InProgress;
    lock.obs_state = ObsState::Recording;
    println!("Recording started from UI.");
}

#[tauri::command]
fn stop_recording(state: tauri::State<AppStatusState>) {
    let mut lock = state.0.lock().unwrap();
    let _ = obs_stop_recording();
    let _ = obs_stop_replay_buffer();
    lock.game_state = GameState::Finished;
    lock.obs_state = ObsState::NotRecording;
    println!("Recording stopped from UI.");
}


struct AppStatusState(Arc<Mutex<AppStatus>>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {

    let status = Arc::new(Mutex::new(AppStatus {
        game_state: GameState::NotStarted,
        obs_state: ObsState::Disconnected,
    }));

    let status_clone = status.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![get_status, start_recording, stop_recording, greet])
        .manage(AppStatusState(status.clone()))
        .setup(move |_app| {

            thread::spawn(move || {
                let mut prev_ingame = false;
                loop {
                    let ingame = poll_lol_ingame();
                    let mut lock = status_clone.lock().unwrap();

                    if ingame && !prev_ingame {

                        let _ = obs_start_recording();
                        let _ = obs_start_replay_buffer();

                        lock.game_state = GameState::InProgress;
                        lock.obs_state = ObsState::Recording;

                        println!("Game started, recording and replay buffer activated.");
                    }
                    else if !ingame && prev_ingame {

                        let _ = obs_stop_recording();
                        let _ = obs_stop_replay_buffer();

                        lock.game_state = GameState::Finished;
                        lock.obs_state = ObsState::NotRecording;
                    }

                    prev_ingame = ingame;
                    drop(lock);
                    thread::sleep(Duration::from_secs(1)); 
                    
                    println!("Current status: {:?}", status_clone.lock().unwrap());
                }
            });
            Ok(())
        })
        
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn poll_lol_ingame() -> bool {
    // This function should implement the logic to check if the game is in progress.
    // For now, we return false as a placeholder.
    true
}

async fn obs_start_recording() -> Result<(), ()> { Ok(()) }
async fn obs_stop_recording() -> Result<(), ()> { Ok(()) }
async fn obs_start_replay_buffer() -> Result<(), ()> { Ok(()) }
async fn obs_stop_replay_buffer() -> Result<(), ()> { Ok(()) }


// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use serde_json::json;
use std::error::Error;
use tauri::async_runtime::spawn;
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;

type WsType = WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>;

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
async fn start_recording(state: tauri::State<'_, AppStatusState>, obs_state: tauri::State<'_, ObsWsState>) -> Result<(), String> {
    {
        let mut lock = state.0.lock().unwrap();
        lock.game_state = GameState::InProgress;
        lock.obs_state = ObsState::Recording;
    }
    spawn(send_obs_command_wrapper(obs_state.0.clone(), "StartRecord"));
    Ok(())
}

#[tauri::command]
async fn stop_recording(state: tauri::State<'_, AppStatusState>, obs_state: tauri::State<'_, ObsWsState>) -> Result<(), String> {
    {
        let mut lock = state.0.lock().unwrap();
        lock.game_state = GameState::Finished;
        lock.obs_state = ObsState::NotRecording;
    }
    spawn(send_obs_command_wrapper(obs_state.0.clone(), "StopRecord"));
    Ok(())
}

#[tauri::command]
async fn start_replay_buffer(state: tauri::State<'_, AppStatusState>, obs_state: tauri::State<'_, ObsWsState>) -> Result<(), String> {
    spawn(send_obs_command_wrapper(obs_state.0.clone(), "StartReplayBuffer"));
    Ok(())
}
#[tauri::command]
async fn stop_replay_buffer(state: tauri::State<'_, AppStatusState>, obs_state: tauri::State<'_, ObsWsState>) -> Result<(), String> {
    spawn(send_obs_command_wrapper(obs_state.0.clone(), "StopReplayBuffer"));
    Ok(())
}
#[tauri::command]
async fn save_replay_buffer(state: tauri::State<'_, AppStatusState>, obs_state: tauri::State<'_, ObsWsState>) -> Result<(), String> {
    spawn(send_obs_command_wrapper(obs_state.0.clone(), "SaveReplayBuffer"));
    Ok(())
}


type SharedObsWsClient = Arc<Mutex<Option<ObsWsClient>>>;
struct ObsWsState(SharedObsWsClient);

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


struct AppStatusState(Arc<Mutex<AppStatus>>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {

    let status = Arc::new(Mutex::new(AppStatus {
        game_state: GameState::NotStarted,
        obs_state: ObsState::Disconnected,
    }));

    // グローバルで
    let obs_ws_client: SharedObsWsClient = Arc::new(Mutex::new(None));

    let status_clone = status.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_status,
            start_recording,
            stop_recording,
            start_replay_buffer,
            stop_replay_buffer,
            save_replay_buffer,
            greet])
        .manage(AppStatusState(status.clone()))
        .manage(ObsWsState(obs_ws_client.clone()))
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
    true
}

async fn obs_start_recording() -> Result<(), ()> { Ok(()) }
async fn obs_stop_recording() -> Result<(), ()> { Ok(()) }
async fn obs_start_replay_buffer() -> Result<(), ()> { Ok(()) }
async fn obs_stop_replay_buffer() -> Result<(), ()> { Ok(()) }


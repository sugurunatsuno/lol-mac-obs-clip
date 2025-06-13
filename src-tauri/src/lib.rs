// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

use serde::{ser, Deserialize, Serialize};
use std::fs::OpenOptions;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use serde_json::json;
use tauri::async_runtime::spawn;
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use reqwest::Client;
use std::collections::HashSet;
use std::io::Write;

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


/// ルート – ゲーム全体
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllGameData {
    pub game_data:    GameData,
    pub events:       EventData,
    pub all_players:  Vec<Player>,
    pub active_player: ActivePlayer,      // 追加
}

/// ゲーム内状態
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameData {
    pub game_time: f64,
    pub game_mode: String,
    pub map_name:  String,
}

/// 現在操作中プレイヤー
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivePlayer {
    pub summoner_name: String,
    pub champion_name: String,
    pub team:          String,
    // ほかに取れるキーがあれば追加可
}

/// イベントラッパ
#[derive(Debug, Clone, Deserialize)]
pub struct EventData {
    #[serde(rename = "Events")]
    pub events: Vec<LolEvent>,
}

/// LoL イベント
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LolEvent {
    pub event_id:   i64,
    pub event_name: String,
    pub event_time: f64,

    // ここからはイベント種別によって存在したりしなかったり
    pub killer_name:  Option<String>,
    pub victim_name:  Option<String>,
    pub assisters:    Option<Vec<String>>,
    pub turret_killed:Option<String>,
    pub inhib_killed: Option<String>,
    pub dragon_type:  Option<String>,
    pub stolen:       Option<String>,   // "False"/"True" 文字列なので String で受ける
    pub kill_streak:  Option<i64>,
    pub acer:         Option<String>,
    pub acing_team:   Option<String>,
}

/// 全プレイヤー情報
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    pub summoner_name: String,
    pub champion_name: String,
    pub team:          String,
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
async fn start_replay_buffer(_state: tauri::State<'_, AppStatusState>, obs_state: tauri::State<'_, ObsWsState>) -> Result<(), String> {
    spawn(send_obs_command_wrapper(obs_state.0.clone(), "StartReplayBuffer"));
    Ok(())
}
#[tauri::command]
async fn stop_replay_buffer(_state: tauri::State<'_, AppStatusState>, obs_state: tauri::State<'_, ObsWsState>) -> Result<(), String> {
    spawn(send_obs_command_wrapper(obs_state.0.clone(), "StopReplayBuffer"));
    Ok(())
}
#[tauri::command]
async fn save_replay_buffer(_state: tauri::State<'_, AppStatusState>, obs_state: tauri::State<'_, ObsWsState>) -> Result<(), String> {
    spawn(send_obs_command_wrapper(obs_state.0.clone(), "SaveReplayBuffer"));
    Ok(())
}

#[tauri::command]
async fn get_saved_directory(_state: tauri::State<'_, AppStatusState>, obs_state: tauri::State<'_, ObsWsState>) -> Result<(), String> {
  spawn(send_obs_command_wrapper(obs_state.0.clone(), "GetRecordDirectory"));
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
            get_saved_directory,
            greet])
        .manage(AppStatusState(status.clone()))
        .manage(ObsWsState(obs_ws_client.clone()))
        .setup(move |_app| {

            // OBS WebSocketクライアントの初期化
            let obs_ws_client_clone = obs_ws_client.clone();

            // LoLのイベントをポーリングして処理するスレッド
            tauri::async_runtime::spawn(async move {
                // LoLのイベントをポーリング
                poll_lol_events(move |all_data: AllGameData| {
                    // ゲーム状態の更新
                    let mut status = status_clone.lock().unwrap();
                    if status.game_state == GameState::NotStarted {
                        status.game_state = GameState::InProgress;
                    }

                    // 現在操作中のプレイヤー情報をログに出力
                    println!("Active Player: {} ({})", all_data.active_player.summoner_name, all_data.active_player.champion_name);

                    // イベントごとに処理
                    for event in all_data.events.events {
                        match event.event_name.as_str() {
                            "ChampionKill" => {
                                println!("{} killed {} (Killer: {}, Victim: {})", event.event_time, event.event_name, event.killer_name.unwrap_or_default(), event.victim_name.unwrap_or_default());
                            }
                            "Multikill" => {
                                // active_player == killer_name の場合、OBSに送信
                                if all_data.active_player.summoner_name == event.killer_name.as_deref().unwrap_or_default() {
                                    let obs_client_clone = obs_ws_client_clone.clone();

                                    tauri::async_runtime::spawn(async move {
                                        if let Err(e) = send_obs_command_wrapper(obs_client_clone, "SaveReplayBuffer").await {
                                            eprintln!("Failed to send Multikill command to OBS: {}", e);
                                        } else {
                                            println!("Sent Multikill command to OBS");
                                        }
                                    });
                                }
                            
                            }
                            "TurretKilled" => {
                                println!("{} turret killed by {}", event.event_time, event.turret_killed.unwrap_or_default());
                            }
                            "DragonKill" => {
                                println!("{} dragon killed: {} (Stolen: {})", event.event_time, event.dragon_type.as_deref().unwrap_or("Unknown"), event.stolen.as_deref().unwrap_or("False"));
                            }
                            _ => {
                                println!("Unhandled event: {} at {}", event.event_name, event.event_time);
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

/// LoLのイベントをポーリングしてコールバックを呼び出す
async fn poll_lol_events<F>(mut callback: F)
where
    F: FnMut(AllGameData) + Send + 'static,
{
    let client = Client::builder()
        .danger_accept_invalid_certs(true) // LoLのローカルAPIは自己署名証明書
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap();
    let mut last_event_ids: HashSet<i64> = HashSet::new();

    println!("Starting LoL event polling...");

    loop {
        println!("Polling LoL events...");

        match client.get("https://127.0.0.1:2999/liveclientdata/allgamedata").send().await {
            Ok(response) => {
                if let Ok(body) = response.text().await {

                    if let Ok(all_data) = serde_json::from_str::<AllGameData>(&body){
                        // 新しいイベントのみを処理
                        let new_events: Vec<LolEvent> = all_data.clone().events.events.into_iter()
                            .filter(|event| !last_event_ids.contains(&event.event_id))
                            .collect();

                        if !new_events.is_empty() {
                            // コールバックを呼び出す
                            callback(all_data.clone());

                            // 新しいイベントIDを記録
                            for event in &new_events {
                                last_event_ids.insert(event.event_id);
                            }
                        }
                    } else {
                        eprintln!("Failed to parse AllGameData from response: {}", body);
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


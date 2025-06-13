// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use serde_json::json;
use tauri::async_runtime::spawn;
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use reqwest::Client;
use std::collections::HashSet;

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
                poll_lol_events(move |all_data: &AllGameData, new_events: Vec<LolEvent>| {
                    // ゲーム状態の更新
                    let mut status = status_clone.lock().unwrap();
                    if status.game_state == GameState::NotStarted {
                        status.game_state = GameState::InProgress;
                    }

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
                                    if killer_name.contains(all_data.activePlayer.summonerName.as_str()) {
                                        continue; // 自分以外のマルチキルは無視
                                    }
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
        
        .run(tauri::generate_context!())
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


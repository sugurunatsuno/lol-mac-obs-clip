//! OBS WebSocket を使って録画開始/停止などの操作を行うモジュール
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream};
// WebSocket 通信に必要なクレート群をインポート

pub type WsType = WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>;

/// OBS の WebSocket クライアント
pub struct ObsWsClient {
    ws: WsType,
}
// 単純なラッパーで WebSocket ストリームを保持

impl ObsWsClient {
    /// OBS WebSocket に接続し Identify ハンドシェイクを行う
    pub async fn connect_and_identify(
        url: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // OBS WebSocket へ接続
        let (mut ws_stream, _) = connect_async(url).await?;
        // 1. Hello を受信
        let msg = ws_stream.next().await;
        if let Some(Ok(Message::Text(text))) = msg {
            let val: serde_json::Value = serde_json::from_str(&text)?;
            if val.get("op").and_then(|v| v.as_u64()) != Some(0) {
                return Err("No Hello from OBS".into());
            }
        } else {
            return Err("Failed to receive Hello".into());
        }
        // 2. Identify を送信
        let identify = json!({
            "op": 1,
            "d": { "rpcVersion": 1, "authentication": null }
        });
        ws_stream
            .send(Message::Text(identify.to_string().into()))
            .await?;
        // 3. Identified を待つ
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

    /// 応答を待たずにコマンドを送信
    pub async fn send_command(
        &mut self,
        command: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // OBS へコマンドを投げるだけの簡易送信
        let req = json!({
            "op": 6,
            "d": {
                "requestType": command,
                "requestId": "tauri-lol-obs-001"
            }
        });
        self.ws.send(Message::Text(req.to_string().into())).await?;
        Ok(()) // エラーは上位で処理
    }

    /// コマンド送信後、レスポンスを待って値を返す
    pub async fn send_request(
        &mut self,
        command: &str,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        // リクエストを投げてレスポンスを待つ
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
            Err("No response".into()) // 応答が無い場合
        }
    }
}

pub type SharedObsWsClient = Arc<Mutex<Option<ObsWsClient>>>;
pub struct ObsWsState(pub SharedObsWsClient);

/// Send OBS command ensuring connection is established
pub async fn send_obs_command_wrapper(
    shared: SharedObsWsClient,
    command: &str,
) -> Result<(), String> {
    // 接続が無い場合は自動で接続を行う
    let mut needs_connect = false;
    {
        let client_guard = shared.lock().unwrap();
        needs_connect = client_guard.is_none();
    }
    if needs_connect {
        // 未接続なら接続と認証を行う
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
        client
            .send_command(command)
            .await
            .map_err(|e| e.to_string())
    } else {
        Err("OBS WS Client not connected".into())
    };
    {
        let mut guard = shared.lock().unwrap();
        *guard = client_opt;
    }
    result // 結果をそのまま返す
}

/// Send OBS command and return the response
pub async fn send_obs_request_wrapper(
    shared: SharedObsWsClient,
    command: &str,
) -> Result<serde_json::Value, String> {
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
        client
            .send_request(command)
            .await
            .map_err(|e| e.to_string())
    } else {
        Err("OBS WS Client not connected".into())
    };
    {
        let mut guard = shared.lock().unwrap();
        *guard = client_opt;
    }
    result // 正常時は Ok(())
}

/// OBS 側の録画保存先ディレクトリを変更する
pub async fn set_record_directory(shared: SharedObsWsClient, dir: &str) -> Result<(), String> {
    // ディレクトリ設定前に接続確認
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
    use serde_json::json;
    use tokio_tungstenite::tungstenite::Message;
    let result = if let Some(ref mut client) = client_opt {
        let req = json!({
            "op": 6,
            "d": {
                "requestType": "SetRecordDirectory",
                "requestId": "tauri-lol-obs-002",
                "requestData": { "recordDirectory": dir }
            }
        });
        client
            .ws
            .send(Message::Text(req.to_string().into()))
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("OBS WS Client not connected".into())
    };
    {
        let mut guard = shared.lock().unwrap();
        *guard = client_opt;
    }
    result
}

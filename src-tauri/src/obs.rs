use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream};
use tokio::net::TcpStream;
use futures_util::{SinkExt, StreamExt};
use std::sync::{Arc, Mutex};

pub type WsType = WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>;

pub struct ObsWsClient {
    ws: WsType,
}

impl ObsWsClient {
    /// Connect to OBS WebSocket and perform Identify handshake
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

    /// Send a command without waiting for response
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

    /// Send a command and wait for response
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

pub type SharedObsWsClient = Arc<Mutex<Option<ObsWsClient>>>;
pub struct ObsWsState(pub SharedObsWsClient);

/// Send OBS command ensuring connection is established
pub async fn send_obs_command_wrapper(shared: SharedObsWsClient, command: &str) -> Result<(), String> {
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
        client.send_command(command).await.map_err(|e| e.to_string())
    } else {
        Err("OBS WS Client not connected".into())
    };
    {
        let mut guard = shared.lock().unwrap();
        *guard = client_opt;
    }
    result
}

/// Send OBS command and return the response
pub async fn send_obs_request_wrapper(shared: SharedObsWsClient, command: &str) -> Result<serde_json::Value, String> {
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


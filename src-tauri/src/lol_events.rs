use std::time::Duration;

use serde::Deserialize;
#[derive(Deserialize, Clone)]
struct Event {
    #[serde(rename = "EventID")]
    id: u64,
    #[serde(rename = "EventName")]
    name: String,
    #[serde(flatten)]
    data: serde_json::Value,
#[derive(Deserialize)]
struct EventData {
    #[serde(rename = "Events")]
    events: Vec<Event>,
/// Polls the live client API on port 2999 and emits Tauri events whenever new LoL events occur.
    let mut last_id = 0u64;
    let url = "https://127.0.0.1:2999/liveclientdata/eventdata";
        let resp = client.get(url).send().await;
        if let Ok(resp) = resp {
            if resp.status().is_success() {
                let data: EventData = resp.json().await?;
                for event in data.events {
                    if event.id > last_id {
                        last_id = event.id;
                        let _ = app.emit_all("lol-event", event.clone());
                    }
                }
    let url = format!("https://127.0.0.1:{}/lol-gameflow/v1/gameflow-phase", port);
    let mut last_phase = String::new();

    loop {
        let resp = client
            .get(&url)
            .basic_auth("riot", Some(&token))
            .send()
            .await?;
        if resp.status().is_success() {
            let phase: String = resp.json().await?;
            if phase != last_phase {
                last_phase = phase.clone();
                let _ = app.emit("lol-event", phase);
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

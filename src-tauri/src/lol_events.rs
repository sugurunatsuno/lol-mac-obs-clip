use std::{fs, path::{Path, PathBuf}, time::Duration};

use tauri::{AppHandle, Emitter};
// use tauri::Manager;

/// Find the League of Legends lockfile on macOS.
fn find_lockfile() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let path = Path::new(&home)
        .join("Library/Application Support/League of Legends/lockfile");
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

/// Read the port and authentication token from the lockfile.
fn read_lockfile(path: &Path) -> Option<(u16, String)> {
    let content = fs::read_to_string(path).ok()?;
    let parts: Vec<&str> = content.split(':').collect();
    if parts.len() >= 5 {
        let port = parts[2].parse().ok()?;
        let token = parts[3].to_string();
        Some((port, token))
    } else {
        None
    }
}

/// Polls the client API and emits Tauri events whenever the gameflow phase changes.
pub async fn listen_for_events(app: AppHandle) -> anyhow::Result<()> {
    let lockfile = find_lockfile().ok_or_else(|| anyhow::anyhow!("Lockfile not found"))?;
    let (port, token) = read_lockfile(&lockfile)
        .ok_or_else(|| anyhow::anyhow!("Failed to read lockfile"))?;

    let client = reqwest::ClientBuilder::new()
        .danger_accept_invalid_certs(true)
        .build()?;

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

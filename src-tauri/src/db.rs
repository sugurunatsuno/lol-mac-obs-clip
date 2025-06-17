use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
use crate::lol::LolEvent;
use serde::Serialize;

#[derive(Clone)]
pub struct DbPath(pub PathBuf);

pub async fn init_db(path: &Path) -> Result<(), String> {
    let p = path.to_path_buf();
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let conn = Connection::open(p).map_err(|e| e.to_string())?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS clips (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT UNIQUE NOT NULL,
                size INTEGER NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                clip_id INTEGER NOT NULL,
                offset REAL NOT NULL,
                event_json TEXT NOT NULL,
                FOREIGN KEY(clip_id) REFERENCES clips(id) ON DELETE CASCADE
            );",
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn write_clip_metadata(
    db_path: &Path,
    video_path: &Path,
    events: &[LolEvent],
    clip_start: f64,
) -> Result<(), String> {
    let db = db_path.to_path_buf();
    let file = video_path.to_path_buf();
    let events_vec = events.to_vec();
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let conn = Connection::open(db).map_err(|e| e.to_string())?;
        let size = std::fs::metadata(&file).map_err(|e| e.to_string())?.len() as i64;
        conn.execute(
            "INSERT OR IGNORE INTO clips (path, size) VALUES (?1, ?2)",
            params![file.to_string_lossy(), size],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE clips SET size=?2 WHERE path=?1",
            params![file.to_string_lossy(), size],
        )
        .map_err(|e| e.to_string())?;
        let clip_id: i64 = conn
            .query_row("SELECT id FROM clips WHERE path=?1", params![file.to_string_lossy()], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM events WHERE clip_id=?1", params![clip_id])
            .map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        {
            let mut stmt = tx
                .prepare("INSERT INTO events (clip_id, offset, event_json) VALUES (?1, ?2, ?3)")
                .map_err(|e| e.to_string())?;
            for ev in events_vec {
                let json = serde_json::to_string(&ev).map_err(|e| e.to_string())?;
                let offset = ev.EventTime - clip_start;
                stmt.execute(params![clip_id, offset, json])
                    .map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
pub struct EventWithOffset {
    #[serde(flatten)]
    pub event: LolEvent,
    pub offset: f64,
}

pub async fn get_clip_events(db_path: &Path, path: &Path) -> Result<Vec<EventWithOffset>, String> {
    let db = db_path.to_path_buf();
    let p = path.to_string_lossy().to_string();
    tokio::task::spawn_blocking(move || -> Result<Vec<EventWithOffset>, String> {
        let conn = Connection::open(db).map_err(|e| e.to_string())?;
        let clip_id: i64 = conn
            .query_row("SELECT id FROM clips WHERE path=?1", params![p], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT offset, event_json FROM events WHERE clip_id=?1 ORDER BY id")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![clip_id], |row| {
                let offset: f64 = row.get(0)?;
                let json: String = row.get(1)?;
                Ok((offset, json))
            })
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for r in rows {
            let (offset, json) = r.map_err(|e| e.to_string())?;
            let event: LolEvent = serde_json::from_str(&json).map_err(|e| e.to_string())?;
            out.push(EventWithOffset { event, offset });
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn list_clips(db_path: &Path) -> Result<Vec<(String, String, u64)>, String> {
    let db = db_path.to_path_buf();
    tokio::task::spawn_blocking(move || -> Result<Vec<(String, String, u64)>, String> {
        let conn = Connection::open(db).map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT path, created_at, size FROM clips ORDER BY created_at DESC")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                let path: String = row.get(0)?;
                let created: String = row.get(1)?;
                let size: i64 = row.get(2)?;
                Ok((path, created, size as u64))
            })
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| e.to_string())?);
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn cleanup_orphan_clips(db_path: &Path) -> Result<(), String> {
    let db = db_path.to_path_buf();
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let conn = Connection::open(db).map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT id, path FROM clips")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let path: String = row.get(1)?;
                Ok((id, path))
            })
            .map_err(|e| e.to_string())?;
        for r in rows {
            let (id, path) = r.map_err(|e| e.to_string())?;
            if !std::path::Path::new(&path).exists() {
                conn.execute("DELETE FROM clips WHERE id=?1", params![id])
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

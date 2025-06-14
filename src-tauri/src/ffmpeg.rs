use tokio::process::{Child, ChildStdout, Command};
use tokio::io::{AsyncBufReadExt, BufReader, AsyncWriteExt};
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;
use std::path::PathBuf;
use nix::sys::signal::{kill, Signal::SIGTERM};
use nix::unistd::Pid;
use tokio_tungstenite; // for MaybeTlsStream? not needed but can't compile? Wait Ffmpeg doesn't use.
use tokio; // not necessary? We'll rely on crate features.
use dirs;

use crate::lol::LolEvent;
use serde::Serialize;
use tokio::fs;

/// Download or locate ffmpeg binary
pub async fn ensure_ffmpeg_path(config: &tauri::Config) -> Result<PathBuf, Box<dyn std::error::Error>> {
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

pub struct FfmpegProcess {
    pub child: Option<Child>,
    pub stdout: Option<BufReader<ChildStdout>>,
    pub ffmpeg_path: PathBuf,
    pub segment_seconds: u32,
    pub video_source: String,
    pub audio_source: String,
    pub fps: u32,
}

impl FfmpegProcess {
    pub fn new(ffmpeg_path: PathBuf) -> Self {
        Self {
            child: None,
            stdout: None,
            ffmpeg_path,
            segment_seconds: 6,
            video_source: "1".into(),
            audio_source: "none".into(),
            fps: 30,
        }
    }

    pub fn set_segment_seconds(&mut self, secs: u32) {
        self.segment_seconds = secs;
    }

    pub fn set_video_source(&mut self, src: String) {
        self.video_source = src;
    }

    pub fn set_ffmpeg_path(&mut self, path: PathBuf) {
        self.ffmpeg_path = path;
    }

    pub fn set_audio_source(&mut self, src: String) {
        self.audio_source = src;
    }

    pub fn set_fps(&mut self, fps: u32) {
        self.fps = fps;
    }

    pub async fn start(&mut self) -> Result<(), String> {
        if self.child.is_some() {
            return Ok(());
        }
        let mut child = Command::new("sh")
            .arg("./ffmpeg_replaybuffer.sh")
            .env("FFMPEG_BIN", &self.ffmpeg_path)
            .arg("-t")
            .arg(self.segment_seconds.to_string())
            .arg("-f")
            .arg(self.fps.to_string())
            .arg("-s")
            .arg(format!("{}:{}", self.video_source, self.audio_source))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        if let Some(out) = child.stdout.take() {
            self.stdout = Some(BufReader::new(out));
        }
        self.child = Some(child);
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<(), String> {
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
        self.stdout = None;
        Ok(())
    }

    pub async fn save(&mut self) -> Result<PathBuf, String> {
        if let Some(child) = &mut self.child {
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(b"s").await.map_err(|e| e.to_string())?;
            }
        } else {
            return Err("ffmpeg not running".into());
        }

        if let Some(stdout) = self.stdout.as_mut() {
            let mut line = String::new();
            loop {
                line.clear();
                let n = stdout
                    .read_line(&mut line)
                    .await
                    .map_err(|e| e.to_string())?;
                if n == 0 {
                    return Err("ffmpeg ended".into());
                }
                if let Some(ts) = line.strip_prefix("saved ") {
                    let ts = ts.trim();
                    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
                    path.push("Movies");
                    path.push(format!("replay_{}.mp4", ts));
                    return Ok(path);
                }
            }
        }
        Err("no stdout".into())
    }
}

pub type SharedFfmpegProcess = Arc<AsyncMutex<FfmpegProcess>>;
pub struct FfmpegState(pub SharedFfmpegProcess);

pub async fn write_clip_metadata(
    path: &std::path::Path,
    events: &[LolEvent],
    clip_start: f64,
) -> Result<(), String> {
    #[derive(Serialize)]
    struct EventWithOffset<'a> {
        #[serde(flatten)]
        event: &'a LolEvent,
        offset: f64,
    }

    #[derive(Serialize)]
    struct Metadata<'a> {
        events: Vec<EventWithOffset<'a>>,
    }

    let events_with_offset = events
        .iter()
        .map(|e| EventWithOffset {
            event: e,
            offset: e.EventTime - clip_start,
        })
        .collect();

    let data = Metadata {
        events: events_with_offset,
    };

    let json_path = path.with_extension("json");
    let contents = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    fs::write(json_path, contents)
        .await
        .map_err(|e| e.to_string())
}


use tauri::api::process::{Command, CommandChild};
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;
use std::path::PathBuf;
use tokio_tungstenite; // maybe required elsewhere
use tokio; // rely on crate features
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
    pub child: Option<CommandChild>,
    pub ram_device: Option<String>,
    pub ram_dir: Option<PathBuf>,
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
            ram_device: None,
            ram_dir: None,
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
        const WRAP: u32 = 11;
        const RAM_MB: u32 = 512;
        const BITRATE: &str = "20M";

        let blocks = RAM_MB * 2048;
        let output = Command::new("hdiutil")
            .args(["attach", "-nomount", &format!("ram://{}", blocks)])
            .output()
            .map_err(|e| e.to_string())?;
        let dev = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Command::new("sudo")
            .args(["diskutil", "erasevolume", "HFS+", "RAMDisk", &dev])
            .status()
            .map_err(|e| e.to_string())?;
        let dir = PathBuf::from("/Volumes/RAMDisk/replay");
        fs::create_dir_all(&dir).await.map_err(|e| e.to_string())?;

        let gop = self.fps * self.segment_seconds;
        let (mut rx, child) = Command::new(&self.ffmpeg_path)
            .args([
                "-f",
                "avfoundation",
                "-pixel_format",
                "nv12",
                "-framerate",
                &(self.fps * 2).to_string(),
                "-i",
                &format!("{}:{}", self.video_source, self.audio_source),
                "-vf",
                &format!("fps={},format=yuv420p", self.fps),
                "-c:v",
                "h264_videotoolbox",
                "-realtime",
                "1",
                "-bf",
                "0",
                "-b:v",
                BITRATE,
                "-g",
                &gop.to_string(),
                "-keyint_min",
                &gop.to_string(),
                "-sc_threshold",
                "0",
                "-force_key_frames",
                &format!("expr:gte(t,n_forced*{}-0.1)", self.segment_seconds),
                "-an",
                "-f",
                "segment",
                "-segment_time",
                &self.segment_seconds.to_string(),
                "-segment_format",
                "ts",
                "-segment_wrap",
                &WRAP.to_string(),
                "-segment_list",
                &dir.join("list.m3u8").to_string_lossy(),
                "-segment_list_size",
                &WRAP.to_string(),
                "-segment_list_type",
                "m3u8",
                "-segment_list_flags",
                "+live",
                &dir.join("seg%03d.ts").to_string_lossy(),
            ])
            .spawn()
            .map_err(|e| e.to_string())?;
        tauri::async_runtime::spawn(async move {
            while rx.recv().await.is_some() {}
        });

        self.child = Some(child);
        self.ram_device = Some(dev);
        self.ram_dir = Some(dir);
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<(), String> {
        if let Some(child) = self.child.take() {
            let _ = child.kill();
        }
        if let Some(dir) = &self.ram_dir {
            let _ = Command::new("diskutil")
                .args(["eject", "/Volumes/RAMDisk"])
                .status();
            if let Some(dev) = &self.ram_device {
                let _ = Command::new("hdiutil").args(["detach", dev]).status();
            }
            let _ = fs::remove_dir_all(dir).await;
        }
        self.ram_device = None;
        self.ram_dir = None;
        Ok(())
    }

    pub async fn save(&mut self) -> Result<PathBuf, String> {
        const WRAP: u32 = 11;

        let dir = if let Some(d) = &self.ram_dir {
            d.clone()
        } else {
            return Err("ffmpeg not running".into());
        };

        let offset = -(WRAP as i32 - 1);
        let dur = self.segment_seconds * (WRAP - 1);
        let ts = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        let mut out = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        out.push("Movies");
        fs::create_dir_all(&out).await.map_err(|e| e.to_string())?;
        out.push(format!("replay_{}.mp4", ts));

        Command::new(&self.ffmpeg_path)
            .args([
                "-nostdin",
                "-y",
                "-live_start_index",
                &offset.to_string(),
                "-i",
                &dir.join("list.m3u8").to_string_lossy(),
                "-t",
                &dur.to_string(),
                "-c",
                "copy",
                "-movflags",
                "+faststart",
                &out.to_string_lossy(),
            ])
            .output()
            .map_err(|e| e.to_string())?;

        Ok(out)
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


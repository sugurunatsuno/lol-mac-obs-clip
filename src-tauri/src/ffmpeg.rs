//! ffmpeg プロセスを制御するラッパーモジュール
//! 録画やリプレイバッファをシェルコマンドで実装する
use std::sync::Arc;
use tauri::App;
use tokio::sync::Mutex as AsyncMutex;
use std::path::PathBuf;
use std::process::{Command};
use std::process::Child as CommandChild;
use tauri::Manager;

use crate::lol::LolEvent;
use tokio::fs;

fn default_save_dir_path() -> PathBuf {
    // ホームディレクトリ直下の Movies フォルダを利用
    let mut dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("Movies");
    dir
}

/// ffmpeg バイナリをダウンロードまたは既存のものを利用
pub async fn ensure_ffmpeg_path(app: &App) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // アプリ用データディレクトリ内に ffmpeg フォルダを確保
    let mut dir = app.path().app_local_data_dir().unwrap();
    dir.push("ffmpeg");
    fs::create_dir_all(&dir).await?;
    let bin_name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let bin_path = dir.join(bin_name);
    if bin_path.exists() {
        return Ok(bin_path);
    }

    // OS/アーキテクチャに合わせてダウンロード URL を選択
    let url = if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffmpeg-mac-arm64"
    } else if cfg!(target_os = "macos") {
        "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffmpeg-mac-x64"
    } else if cfg!(target_os = "windows") {
        "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffmpeg-win32-x64.exe"
    } else {
        "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffmpeg-linux-x64"
    };

    // ffmpeg バイナリをダウンロードして保存
    let bytes = reqwest::get(url).await?.bytes().await?;
    fs::write(&bin_path, &bytes).await?;

#[cfg(unix)]
{
    use std::os::unix::fs::PermissionsExt;
    // 実行権限を付与しておく
    let mut perm = fs::metadata(&bin_path).await?.permissions();
    perm.set_mode(0o755);
    fs::set_permissions(&bin_path, perm).await?;
}

    Ok(bin_path)
}

/// ffmpeg コマンド実行の状態を保持
/// RAM ディスクやバッファ設定もここで管理する
pub struct FfmpegProcess {
    pub child: Option<CommandChild>,
    pub ram_device: Option<String>,
    pub ram_dir: Option<PathBuf>,
    pub ffmpeg_path: PathBuf,
    pub segment_seconds: u32,
    pub video_source: String,
    pub audio_source: String,
    pub fps: u32,
    pub wrap_count: u32,
    pub bitrate: String,
    pub save_dir: PathBuf,
}

impl FfmpegProcess {
    pub fn new(ffmpeg_path: PathBuf) -> Self {
        // デフォルト値を設定して初期化
        Self {
            child: None,
            ram_device: None,
            ram_dir: None,
            ffmpeg_path,
            segment_seconds: 6,
            video_source: "1".into(),
            audio_source: "none".into(),
            fps: 30,
            wrap_count: 11,
            bitrate: "20M".into(),
            save_dir: default_save_dir_path(),
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

    pub fn set_wrap_count(&mut self, wrap: u32) {
        self.wrap_count = wrap;
    }

    pub fn set_bitrate(&mut self, bitrate: String) {
        self.bitrate = bitrate;
    }

    pub fn set_save_dir(&mut self, dir: PathBuf) {
        self.save_dir = dir;
    }

    pub async fn start(&mut self, shared: SharedFfmpegProcess) -> Result<(), String> {
        // ffmpeg プロセスを起動し循環バッファを構築
        // 既に動いている場合は何もしない
        if let Some(child) = self.child.as_mut() {
            if child.try_wait().map_err(|e| e.to_string())?.is_none() {
                return Ok(());
            }
        }
        if self.child.is_some() {
            self.stop().await?;
        }
        const WRAP: u32 = 11;
        const BITRATE: &str = "20M";

        // セグメント保存用ディレクトリを作成
        let dir = PathBuf::from("/tmp/lol_obs_clip/replay");
        if dir.exists() {
            fs::remove_dir_all(&dir).await.map_err(|e| e.to_string())?;
        }
        fs::create_dir_all(&dir).await.map_err(|e| e.to_string())?;

        // GOP 長をセグメント長と FPS から計算
        let gop = self.fps * self.segment_seconds;
        let child = Command::new(&self.ffmpeg_path)
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
                &self.bitrate,
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
                &self.wrap_count.to_string(),
                "-segment_list",
                &dir.join("list.m3u8").to_string_lossy(),
                "-segment_list_size",
                &self.wrap_count.to_string(),
                "-segment_list_type",
                "m3u8",
                "-segment_list_flags",
                "+live",
                &dir.join("seg%03d.ts").to_string_lossy(),
            ])
            .spawn() // ffmpeg プロセス開始
            .map_err(|e| e.to_string())?;

        self.child = Some(child);

        self.ram_dir = Some(dir.clone());

        let save_path = dir.join(".trigger_save");
        let quit_path = dir.join(".trigger_quit");
        // ファイルトリガー監視タスクを起動
        tauri::async_runtime::spawn(async move {
            use tokio::time::{sleep, Duration};
            loop {
                if fs::metadata(&save_path).await.is_ok() {
                    let _ = fs::remove_file(&save_path).await;
                    let mut p = shared.lock().await;
                    let _ = p.save().await;
                }
                if fs::metadata(&quit_path).await.is_ok() {
                    let _ = fs::remove_file(&quit_path).await;
                    let mut p = shared.lock().await;
                    let _ = p.stop().await;
                    break;
                }
                sleep(Duration::from_millis(250)).await;
            }
        });

        Ok(())
    }

    pub async fn stop(&mut self) -> Result<(), String> {
        // ffmpeg プロセスと一時ディレクトリを後始末
        if let Some(mut child) = self.child.take() {
            let _ = child.kill(); // プロセス終了を試みる
        }
        if let Some(dir) = &self.ram_dir {
            let _ = fs::remove_dir_all(dir).await;
        }
        self.ram_device = None;
        self.ram_dir = None;
        Ok(())
    }

    pub async fn save(&mut self) -> Result<PathBuf, String> {
        // 現在のバッファ内容を mp4 として保存

        let dir = if let Some(d) = &self.ram_dir {
            d.clone()
        } else {
            return Err("ffmpeg not running".into());
        };

        let offset = -(self.wrap_count as i32 - 1);
        let dur = self.segment_seconds * (self.wrap_count - 1);
        let ts = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        let mut out = self.save_dir.clone();
        fs::create_dir_all(&out).await.map_err(|e| e.to_string())?;
        out.push(format!("replay_{}.mp4", ts)); // 保存先ファイル名を決定

        // ffmpeg を呼び出してクリップを出力
        let output = Command::new(&self.ffmpeg_path)
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
        if !output.status.success() {
            // ffmpeg が失敗した場合は stderr を返す
            return Err(String::from_utf8_lossy(&output.stderr).to_string());
        }

        Ok(out)
    }
}

/// 複数タスク間で ffmpeg プロセスを共有するための型
pub type SharedFfmpegProcess = Arc<AsyncMutex<FfmpegProcess>>;
pub struct FfmpegState(pub SharedFfmpegProcess);

pub async fn write_clip_metadata(
    db_path: &std::path::Path,
    path: &std::path::Path,
    events: &[LolEvent],
    clip_start: f64,
) -> Result<(), String> {
    // 保存したクリップに紐づくイベント情報をDBへ書き込む
    crate::db::write_clip_metadata(db_path, path, events, clip_start).await
}


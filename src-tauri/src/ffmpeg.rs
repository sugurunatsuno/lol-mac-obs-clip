//! ffmpeg プロセスを制御するラッパーモジュール
//! 録画やリプレイバッファをシェルコマンドで実装する
use std::sync::Arc;
use tauri::App;
use tokio::sync::Mutex as AsyncMutex;
use std::path::PathBuf;
use std::process::{Command};
use std::process::Child as CommandChild;
use tauri::Manager;
// ffmpeg 制御に必要な標準ライブラリと Tauri の型をインポート

use crate::lol::LolEvent;
use tokio::fs;

fn default_save_dir_path() -> PathBuf {
    // ホームディレクトリ直下の Movies フォルダを利用
    let mut dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("Movies");
    dir
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
// ffmpeg 実行に必要な状態をまとめた構造体

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
    // 各種 setter でパラメータを変更可能

    pub fn set_segment_seconds(&mut self, secs: u32) {
        // 1 セグメントの長さを更新
        self.segment_seconds = secs;
    }

    pub fn set_video_source(&mut self, src: String) {
        // キャプチャするビデオデバイス番号
        self.video_source = src;
    }

    pub fn set_ffmpeg_path(&mut self, path: PathBuf) {
        // 利用する ffmpeg 実行ファイルを差し替え
        self.ffmpeg_path = path;
    }

    pub fn set_audio_source(&mut self, src: String) {
        // キャプチャするオーディオデバイス番号
        self.audio_source = src;
    }

    pub fn set_fps(&mut self, fps: u32) {
        // 出力フレームレートを設定
        self.fps = fps;
    }

    pub fn set_wrap_count(&mut self, wrap: u32) {
        // 保持するセグメント数を設定
        self.wrap_count = wrap;
    }

    pub fn set_bitrate(&mut self, bitrate: String) {
        // エンコード時のビットレート
        self.bitrate = bitrate;
    }

    pub fn set_save_dir(&mut self, dir: PathBuf) {
        println!("Set save directory to {:?}", dir);
        // クリップ保存先ディレクトリ
        self.save_dir = dir;
    }

    /// ffmpeg プロセスを起動し、録画バッファを開始する
    pub async fn start(&mut self, shared: SharedFfmpegProcess) -> Result<(), String> {
        // ffmpeg プロセスを起動し循環バッファを構築
        // 既に動いている場合は何もしない
        if let Some(child) = self.child.as_mut() {
            if child.try_wait().map_err(|e| e.to_string())?.is_none() {
                println!("ffmpeg process already running");
                return Ok(());
            }
            println!("ffmpeg process was stopped, restarting");
        }

        if self.child.is_some() {
            self.stop().await?;
        }
        const WRAP: u32 = 11;
        const BITRATE: &str = "20M";

        // セグメント保存用ディレクトリを作成
        let dir = PathBuf::from("/tmp/lol_obs_clip/replay");
        println!(
            "Creating segment directory {:?} (segment_seconds={}, wrap_count={}, fps={}, bitrate={})",
            dir, self.segment_seconds, self.wrap_count, self.fps, self.bitrate
        );
        if dir.exists() {
            fs::remove_dir_all(&dir).await.map_err(|e| e.to_string())?;
        }
        fs::create_dir_all(&dir).await.map_err(|e| e.to_string())?;

        // GOP 長をセグメント長と FPS から計算
        let gop = self.fps * self.segment_seconds;
        let mut args: Vec<String> = Vec::new();
        args.push("-f".into());
        args.push("avfoundation".into());
        args.push("-pixel_format".into());
        args.push("nv12".into());
        args.push("-framerate".into());
        args.push((self.fps * 2).to_string());
        args.push("-i".into());
        args.push(format!("{}:{}", self.video_source, self.audio_source));
        args.push("-vf".into());
        args.push(format!("fps={},format=yuv420p", self.fps));
        args.push("-c:v".into());
        args.push("h264_videotoolbox".into());
        args.push("-realtime".into());
        args.push("1".into());
        args.push("-bf".into());
        args.push("0".into());
        args.push("-b:v".into());
        args.push(self.bitrate.clone());
        args.push("-g".into());
        args.push(gop.to_string());
        args.push("-keyint_min".into());
        args.push(gop.to_string());
        args.push("-sc_threshold".into());
        args.push("0".into());
        args.push("-force_key_frames".into());
        args.push(format!("expr:gte(t,n_forced*{}-0.1)", self.segment_seconds));
        args.push("-an".into());
        args.push("-f".into());
        args.push("segment".into());
        args.push("-segment_time".into());
        args.push(self.segment_seconds.to_string());
        args.push("-segment_format".into());
        args.push("ts".into());
        args.push("-segment_wrap".into());
        args.push(self.wrap_count.to_string());
        args.push("-segment_list".into());
        args.push(dir.join("list.m3u8").to_string_lossy().into_owned());
        args.push("-segment_list_size".into());
        args.push(self.wrap_count.to_string());
        args.push("-segment_list_type".into());
        args.push("m3u8".into());
        args.push("-segment_list_flags".into());
        args.push("+live".into());
        args.push(dir.join("seg%03d.ts").to_string_lossy().into_owned());
        println!("Running ffmpeg command: {:?} {:?}", self.ffmpeg_path, args);
        let child = Command::new(&self.ffmpeg_path)
            .args(&args)
            .spawn() // ffmpeg プロセス開始
            .map_err(|e| e.to_string())?;

        self.child = Some(child);
        // プロセスハンドルを保存しておく

        self.ram_dir = Some(dir.clone());
        // 一時保存用ディレクトリのパス

        let save_path = dir.join(".trigger_save");
        let quit_path = dir.join(".trigger_quit");
        // 保存や停止を外部から指示するための監視タスク
        tauri::async_runtime::spawn(async move {
            use tokio::time::{sleep, Duration};
            loop {
                if fs::metadata(&save_path).await.is_ok() {
                    // .trigger_save を検知したらバッファを保存
                    println!("Save trigger detected, saving current buffer");
                    let _ = fs::remove_file(&save_path).await;
                    let mut p = shared.lock().await;
                    let _ = p.save().await;
                }
                if fs::metadata(&quit_path).await.is_ok() {
                    // .trigger_quit を検知したらプロセスを終了
                    println!("Quit trigger detected, stopping ffmpeg process");
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
            println!("Stopping ffmpeg process");
            let _ = child.kill(); // プロセス終了を試みる
        }
        if let Some(dir) = &self.ram_dir {
            // 作成した一時ディレクトリを削除
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

        let offset = -(self.wrap_count as i32 - 1); // ラップしている分の開始位置
        let dur = self.segment_seconds * (self.wrap_count - 1);
        let ts = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        let mut out = self.save_dir.clone();
        fs::create_dir_all(&out).await.map_err(|e| e.to_string())?;
        out.push(format!("replay_{}.mp4", ts)); // 保存先ファイル名を決定

        // ffmpeg を呼び出してクリップを出力
        println!(
            "Saving replay buffer to {:?} (offset={}, duration={})",
            out, offset, dur
        );
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
            .output() // 実際に ffmpeg を実行
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            // ffmpeg が失敗した場合は stderr を返す
            return Err(String::from_utf8_lossy(&output.stderr).to_string());
        }

        println!("Saved clip to {:?}", out);
        // 正常終了した場合は保存先パスを返す

        Ok(out)
    }
}

/// 複数タスク間で ffmpeg プロセスを共有するための型
pub type SharedFfmpegProcess = Arc<AsyncMutex<FfmpegProcess>>;
pub struct FfmpegState(pub SharedFfmpegProcess);
// アプリ全体で共有するためのラッパー型

pub async fn write_clip_metadata(
    db_path: &std::path::Path,
    path: &std::path::Path,
    events: &[LolEvent],
    clip_start: f64,
) -> Result<(), String> {
    // 保存したクリップに紐づくイベント情報をDBへ書き込む
    // crate::db::write_clip_metadata(db_path, path, events, clip_start).await
    Ok(())
}


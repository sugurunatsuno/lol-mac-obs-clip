# LoL OBS Clip Tool

This project provides a minimal Tauri application for capturing League of Legends highlights through OBS.
The frontend uses [Tailwind CSS](https://tailwindcss.com/) loaded via CDN for styling.

## Prerequisites

- **Rust**: install the latest stable toolchain via [rustup](https://rust-lang.org/tools/install).
- **pnpm**: used for managing JavaScript dependencies.
- **OBS Studio** with the **WebSocket** plugin enabled (v5 or later recommended).
- No manual **ffmpeg** installation is required. The app downloads a suitable
  binary to its local data directory on first run.
- A running **League of Legends** client when using the tool.

## Build and Run

Install JavaScript dependencies and run the app in development mode:

```bash
pnpm install
pnpm tauri dev
```

For a release build:

```bash
pnpm tauri build
```

The commands above require that the Rust toolchain is installed since Tauri compiles a Rust backend.

## OBS Configuration

1. Install the OBS WebSocket plugin and restart OBS.
2. Open *Tools → WebSocket Server Settings* and enable the server (default port `4455`).
3. Configure your recording or replay buffer path in OBS or set it from the application's settings.
4. Use the same port and password (if set) in this application's settings so it can control OBS.

## League of Legends Setup

Start the LoL client before launching the app. The client exposes an API that this tool queries; no additional configuration is normally required.

## Usage

1. Launch OBS and ensure the WebSocket server is running.
2. Start the League of Legends client and this application.
3. When a highlight occurs, press the **Clip** button (or configured hotkey) to save the OBS replay buffer.
4. Saved videos can be opened from within the app for playback.
5. Each clip is accompanied by a `replay_<timestamp>.json` file listing all LoL events
   from the clip duration along with their offsets from the start of the video.
6. Optionally choose to encode clips with ffmpeg for easier sharing.
7. Use the **設定保存** button to persist your current options. They are loaded automatically on startup. You can also set the folder where recordings and clips are saved from the settings screen. The same path is used for OBS recordings and ffmpeg clips.

## `ffmpeg_replaybuffer.sh` (macOS)

This repository includes a helper script for capturing a rolling replay buffer
with ffmpeg. The script only works on **macOS** because it relies on the
`avfoundation` input device and it creates a RAM disk using `hdiutil`. The RAM
disk creation requires `sudo` so be prepared to enter your password when you run
the script.

Example usage:

```bash
sudo ./ffmpeg_replaybuffer.sh -f 30 -b 20M -s "1:none" -t 6 -n 11 -r 512
```

Parameters:

- `-f FPS` – output frames per second (default `30`).
- `-b BITRATE` – target video bitrate (default `20M`).
- `-s SRC` – capture source for avfoundation (default `"1:none"`).
- `-t SEG_S` – length of each segment in seconds (default `6`).
- `-n WRAP` – number of segments kept in the buffer (default `11`).
- `-r RAM_MB` – size of the RAM disk in megabytes (default `512`).
- `-o OUT_DIR` – directory where saved clips are written (default `~/Movies`).

While the script is running it displays `REC ▶︎`. Press **s** at any time to
save the most recent replay buffer to the specified output directory (default `~/Movies`) as
`replay_<timestamp>.mp4`. Press **Esc** or **q** to quit.

Once integrated with the Tauri app, the application will spawn this script
automatically so you can manage the replay buffer from the GUI. Until then you
can invoke the script manually from the project directory.

### Replay buffer options

When starting the replay buffer from the Tauri application, you can override several
`ffmpeg_replaybuffer.sh` options:

- `segment_seconds` (`-t`) – length of each segment in seconds. Defaults to `6`.
- `fps` (`-f`) – output frames per second. Defaults to `30`.
- `video_source`/`audio_source` (`-s`) – combined as `<video>:<audio>` for the
  `avfoundation` input. Defaults to `1:none`.

These values are passed directly to `ffmpeg_replaybuffer.sh` when it is spawned.

While the replay buffer is running, creating a `.trigger_save` file inside the
segment directory will cause the buffer to be saved. Creating a `.trigger_quit`
file will stop the replay buffer entirely. Both files are removed automatically
after they are processed.

### ffmpeg download location

The application stores its own ffmpeg binary under the directory returned by
`app_local_data_dir`. On macOS this is typically
`~/Library/Application Support/<app>/ffmpeg/ffmpeg`. Replace the binary there to
update or override the version shipped by default.

## Screen and Function Mapping

The frontend now has **five** HTML pages: Home, OBS Mode, FFmpeg Mode, Saved Videos, and Video Detail. Each page links to the others via the shared navigation bar.

| Screen | File | Transitions | Invoked Commands |
|-------|------|-------------|------------------|
| **Home** | `index.html` | Links to **OBS Mode**, **FFmpeg Mode**, **Videos** | – |
| **OBS Mode** | `obs.html` | Navbar links to all screens | `get_status`, `start_recording`, `stop_recording`, `start_replay_buffer`, `stop_replay_buffer`, `save_replay_buffer`, `load_settings_cmd`, `save_settings_cmd`, `set_saved_directory`, `get_saved_directory` |
| **FFmpeg Mode** | `ffmpeg.html` | Navbar links to all screens | `get_status`, `start_recording`, `stop_recording`, `start_ffmpeg_replay`, `stop_ffmpeg_replay`, `save_ffmpeg_clip`, `list_ffmpeg_devices`, `load_settings_cmd`, `save_settings_cmd`, `set_saved_directory`, `get_saved_directory` |
| **Saved Videos** | `videos.html` | Navbar links to all screens. Entries open **Video Detail**. | `list_saved_videos` |
| **Video Detail** | `video.html` | Navbar links to all screens | – (uses `convertFileSrc` for local playback) |

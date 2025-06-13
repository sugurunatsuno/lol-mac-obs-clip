# LoL OBS Clip Tool

This project provides a minimal Tauri application for capturing League of Legends highlights through OBS.

## Prerequisites

- **Rust**: install the latest stable toolchain via [rustup](https://rust-lang.org/tools/install).
- **pnpm**: used for managing JavaScript dependencies.
- **OBS Studio** with the **WebSocket** plugin enabled (v5 or later recommended).
- **ffmpeg** in your `PATH` if you want to encode recordings.
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
3. Configure your recording or replay buffer path in OBS.
4. Use the same port and password (if set) in this application's settings so it can control OBS.

## League of Legends Setup

Start the LoL client before launching the app. The client exposes an API that this tool queries; no additional configuration is normally required.

## Usage

1. Launch OBS and ensure the WebSocket server is running.
2. Start the League of Legends client and this application.
3. When a highlight occurs, press the **Clip** button (or configured hotkey) to save the OBS replay buffer.
4. Saved videos can be opened from within the app for playback.
5. Optionally choose to encode clips with ffmpeg for easier sharing.

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

While the script is running it displays `REC ▶︎`. Press **s** at any time to
save the most recent replay buffer to `$HOME/Movies` as
`replay_<timestamp>.mp4`. Press **Esc** or **q** to quit.

Once integrated with the Tauri app, the application will spawn this script
automatically so you can manage the replay buffer from the GUI. Until then you
can invoke the script manually from the project directory.


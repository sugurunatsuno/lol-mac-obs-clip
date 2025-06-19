# LoL OBS クリップツール

このプロジェクトは、OBS を通じて League of Legends のハイライトをキャプチャするための最小限の Tauri アプリケーションです。フロントエンドには [Tailwind CSS](https://tailwindcss.com/) を CDN 経由で読み込んで使用しています。

## 必要条件

- **Rust**: 最新の安定版ツールチェーンを [rustup](https://rust-lang.org/tools/install) からインストールしてください。
- **pnpm**: JavaScript の依存関係管理に使用します。
- **OBS Studio** と **WebSocket** プラグイン（バージョン 5 以降推奨）。
- **ffmpeg**: `PATH` に通ったコマンドを利用します。事前にインストールしてください。
- ツール利用時は **League of Legends** クライアントを起動しておいてください。

## ビルドと実行

JavaScript の依存関係をインストールし、開発モードでアプリを起動します:

```bash
pnpm install
pnpm tauri dev
```

リリースビルドを作成する場合:

```bash
pnpm tauri build
```

上記コマンドを実行するには Rust ツールチェーンが必要です。Tauri はバックエンドに Rust をコンパイルします。

## OBS の設定

1. OBS WebSocket プラグインをインストールして OBS を再起動します。
2. *Tools → WebSocket Server Settings* を開き、サーバーを有効にします（デフォルトポートは `4455`）。
3. OBS で録画またはリプレイバッファの保存先を設定するか、アプリの設定画面から指定します。
4. アプリの設定でも同じポートとパスワード（設定している場合）を指定して OBS を操作できるようにします。

## League of Legends の準備

アプリを起動する前に LoL クライアントを起動してください。クライアントが公開している API をこのツールが利用するため、通常それ以外の設定は不要です。

## 使い方

1. OBS を起動し、WebSocket サーバーが実行されていることを確認します。
2. League of Legends クライアントと本アプリを起動します。
3. 設定画面の「シェル録画を使用」スイッチで、OBS 録画と ffmpeg 録画を切り替えられます。
4. ハイライトが起きたら **Clip** ボタン（または設定したホットキー）を押してリプレイバッファを保存します。
5. 保存された動画はアプリ内から再生できます。
6. 各クリップには `replay_<timestamp>.json` が付属し、クリップ時間中の全ての LoL イベントとその動画開始からのオフセットが記録されます。
7. 必要に応じて ffmpeg でエンコードして共有しやすい形式に変換できます。
8. **設定保存** ボタンで現在の設定を保存できます。起動時に自動で読み込まれます。録画やクリップの保存先フォルダも設定画面から変更可能で、OBS の録画と ffmpeg クリップのどちらにも同じパスが使用されます。

## `ffmpeg_replaybuffer.sh`（macOS）

このリポジトリには、ffmpeg を用いたロール式リプレイバッファを取得するための補助スクリプトが含まれています。**macOS** 専用で、`avfoundation` 入力デバイスを利用し、`hdiutil` で RAM ディスクを作成します。RAM ディスク作成には `sudo` が必要なため、実行時にパスワード入力を求められます。

使用例:

```bash
sudo ./ffmpeg_replaybuffer.sh -f 30 -b 20M -s "1:none" -t 6 -n 11 -r 512
```

引数:

- `-f FPS` – 出力フレームレート（デフォルト `30`）。
- `-b BITRATE` – 目標ビットレート（デフォルト `20M`）。
- `-s SRC` – avfoundation 用のキャプチャソース（デフォルト `"1:none"`）。
- `-t SEG_S` – セグメントの長さ（秒、デフォルト `6`）。
- `-n WRAP` – バッファに保持するセグメント数（デフォルト `11`）。
- `-r RAM_MB` – RAM ディスクサイズ（MB、デフォルト `512`）。
- `-o OUT_DIR` – クリップ保存先ディレクトリ（デフォルト `~/Movies`）。

スクリプト実行中は `REC ▶︎` と表示されます。任意のタイミングで **s** を押すと、直近のリプレイバッファが指定ディレクトリ（デフォルト `~/Movies`）に `replay_<timestamp>.mp4` として保存されます。**Esc** または **q** で終了します。

この機能は現在 Tauri アプリにも組み込まれており、設定で「シェル録画」を選択すると GUI からリプレイバッファを制御できます。スクリプトは単体で利用したい場合にのみ手動で実行してください。

### リプレイバッファのオプション

Tauri アプリからリプレイバッファを開始する際に、`ffmpeg_replaybuffer.sh` のいくつかのオプションを上書きできます:

- `segment_seconds` (`-t`) – セグメントの長さ（秒）。デフォルトは `6`。
- `fps` (`-f`) – 出力フレームレート。デフォルトは `30`。
- `video_source`/`audio_source` (`-s`) – `<video>:<audio>` の形で `avfoundation` 入力に渡します。デフォルトは `1:none`。

これらの値はスクリプト起動時にそのまま渡されます。

リプレイバッファ実行中にセグメントディレクトリ内に `.trigger_save` ファイルを作成するとバッファが保存されます。`.trigger_quit` ファイルを作成するとリプレイバッファが停止します。どちらのファイルも処理後に自動で削除されます。

### ffmpeg の利用

アプリには ffmpeg バイナリは同梱されていません。`PATH` 上にある `ffmpeg` コマンドをそのまま呼び出して処理を行います。

## 画面と機能の対応

フロントエンドは現在 **5** つの HTML ページで構成されます：Home、OBS モード、FFmpeg モード、Saved Videos、Video Detail。各ページは共通のナビゲーションバーで相互に移動できます。

| Screen | File | Transitions | Invoked Commands |
|-------|------|-------------|------------------|
| **Home** | `index.html` | **OBS Mode**、**FFmpeg Mode**、**Videos** へのリンク | – |
| **OBS Mode** | `obs.html` | ナビバーから全画面へ | `get_status`, `start_recording`, `stop_recording`, `start_replay_buffer`, `stop_replay_buffer`, `save_replay_buffer`, `load_settings_cmd`, `save_settings_cmd`, `set_saved_directory`, `get_saved_directory` |
| **FFmpeg Mode** | `ffmpeg.html` | ナビバーから全画面へ | `get_status`, `start_recording`, `stop_recording`, `start_ffmpeg_replay`, `stop_ffmpeg_replay`, `save_ffmpeg_clip`, `list_ffmpeg_devices`, `load_settings_cmd`, `save_settings_cmd`, `set_saved_directory`, `get_saved_directory` |
| **Saved Videos** | `videos.html` | ナビバーから全画面へ。項目をクリックすると **Video Detail** を開きます | `list_saved_videos` |
| **Video Detail** | `video.html` | ナビバーから全画面へ | – (`convertFileSrc` を使用してローカル再生) |

ナビバー右端には現在の録画モードや録画中かどうかといったデバッグ用ステータスが表示されます。OBS を利用している場合は `OBS`、シェル録画 (ffmpeg) を利用している場合は `ffmpeg` と表示されるので、動作確認に活用してください。


## デバッグ実行

VSCode や JetBrains RustRover でデバッグを行う場合、`run-configs` ディレクトリにサンプルの実行設定を用意しています。必要に応じて以下の手順でコピーしてください。

### VSCode

```
cp run-configs/vscode-launch.json .vscode/launch.json
```

### RustRover

```
mkdir -p .idea/runConfigurations
cp run-configs/rustrover/TauriDev.run.xml .idea/runConfigurations/
```


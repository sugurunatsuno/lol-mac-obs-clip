# 設計書

## 1. 全体構成
本アプリは Tauri を用いたデスクトップアプリケーションであり、フロントエンドを HTML/JavaScript、バックエンドを Rust で実装している。OBS WebSocket もしくは ffmpeg を通じて録画を制御し、League of Legends クライアントの API をポーリングしてイベントを検出する。

```
+-------------+        +--------------+        +---------------------+
| Frontend    | <----> | Tauri Backend| <----> | LoL Client / OBS /  |
| (HTML/JS)   |        |   (Rust)     |        | ffmpeg process      |
+-------------+        +--------------+        +---------------------+
```

## 2. モジュール概要
- `lib.rs` : アプリ全体の状態管理とコマンド定義を行う中心モジュール。
- `obs.rs` : OBS WebSocket への接続とコマンド送信を担当【F:src-tauri/src/obs.rs†L1-L71】。
- `ffmpeg.rs` : ffmpeg をプロセスとして起動し、リプレイバッファや保存処理を行う【F:src-tauri/src/ffmpeg.rs†L1-L159】【F:src-tauri/src/ffmpeg.rs†L161-L226】。
- `settings.rs` : JSON 形式で設定を保存・読み込みする【F:src-tauri/src/settings.rs†L1-L40】。
- `lol.rs` : LoL クライアント API のレスポンスを受け取るためのデータ構造定義【F:src-tauri/src/lol.rs†L1-L194】。
- `db.rs` : クリップに紐づくイベント情報を動画ファイル横の JSON として保存する実装【F:src-tauri/src/db.rs†L17-L39】。

## 3. 主な処理フロー
1. **アプリ起動時**
   - 設定ファイルを読み込み、`AppStatus` を初期化する【F:src-tauri/src/lib.rs†L525-L579】。
   - LoL クライアント API のポーリングタスクを起動し、ゲームイベントを監視する【F:src-tauri/src/lib.rs†L620-L641】。
2. **イベント検出時**
   - `Multikill` など特定イベントでリプレイバッファ保存をトリガーする【F:src-tauri/src/lib.rs†L628-L704】。
   - ffmpeg モードの場合、保存した動画に対してイベントのオフセット情報を JSON へ書き込む【F:src-tauri/src/db.rs†L17-L39】。
3. **フロントエンド操作**
   - `settings.js` から `invoke` を通じて `start_recording` などのコマンドを呼び出す【F:src/settings.js†L20-L70】。
   - 動画一覧取得や再生は `videos.js` と `viewer.js` で実装される【F:src/videos.js†L1-L36】【F:src/viewer.js†L1-L71】。
4. **通知処理**
   - 録画・リプレイバッファの開始/終了/保存時にデスクトップ通知を表示してユーザーへ状態変化を知らせる。
5. **セッション管理**
   - `GameStart` イベントから `GameEnd` までを 1 セッションとし、セッション中に保存した動画ファイルをリスト化する。

## 4. データ保存
- 動画ファイルはユーザー指定ディレクトリに `replay_<timestamp>.mp4` として保存される。
- 付随するイベント情報は同名の `json` ファイルに `EventID` や発生時刻オフセット付きで保存される【F:src-tauri/src/db.rs†L17-L39】。

## 5. 画面構成
- `index.html` : ホーム画面。モード選択リンクのみ【F:src/index.html†L1-L27】。
- `obs.html` : OBS 録画向け操作画面【F:src/obs.html†L13-L60】。
- `videos.html` : 保存済み動画一覧を表示【F:src/videos.html†L1-L35】。
- `video.html` : 動画再生とイベント表示【F:src/video.html†L1-L32】。

## 6. 拡張ポイント
- `ffmpeg_replaybuffer.sh` を単体でも利用できるよう提供しており、アプリの ffmpeg モードでも同等の処理を行う【F:ffmpeg_replaybuffer.sh†L1-L63】。
- `list_device.sh` で各 OS の録画デバイス一覧を取得可能【F:list_device.sh†L1-L26】。


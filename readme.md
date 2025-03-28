# LoL Replay Trigger for OBS

League of Legends の Live Client API を活用し、ゲーム内イベントに応じて自動的に OBS のリプレイバッファを保存するツールです。

## 特徴

- LoLの各種イベントに反応して自動で録画保存！
- イベント種別ごとに録画ON/OFFを設定ファイルから制御可能！
- 自分がデスした時の録画も対応（設定によって有効化可能）
- OBS WebSocket v5 に対応
- 録画保存時にシャッター音を再生
- 画面上の簡易オーバーレイボタン付き（macOS対応）
- セルフホステッド環境でも利用可能

## 対応環境

- Python 3.10以上
- macOS（オーディオ再生には `afplay` 使用）
- OBS + WebSocket Plugin（ポート 4455）

## ダウンロード

ビルド済み実行ファイル（macOS用）は以下からダウンロードできます：

→ [Releasesページ](https://github.com/sugurunatsuno/lol-mac-obs-clip/releases)

最新版には `.zip` にまとめられた実行ファイル（例: `lol-replay-trigger-mac.zip`）が含まれています。


## Tips

- macOS のセキュリティ設定により、初回起動時は許可が必要な場合があります。
- 実行には OBS が起動しており、OBS WebSocket がポート4455で待機している必要があります。
- config.json はアプリと同じディレクトリに配置してください。
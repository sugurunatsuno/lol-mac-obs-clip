#!/usr/bin/env bash
# 各プラットフォームで利用可能な録画デバイス一覧を表示
set -euo pipefail

# 実行環境の OS 名を取得
OS="$(uname -s)"

if [[ "$OS" == "Darwin" ]]; then
    # macOS用: AVFoundationデバイス一覧取得＆パース
    ffmpeg -f avfoundation -list_devices true -i "" 2>&1 \
    | grep -E "^\[AVFoundation (audio|video) devices\]|\[[0-9]+\]" \
    | sed -E 's/^\[AVFoundation video devices\]/Video devices:/;s/^\[AVFoundation audio devices\]/Audio devices:/;s/^\[[0-9]+\] //;s/^[[:space:]]+//'
    # ここでは ffmpeg の出力を加工して一覧表示するだけ
elif [[ "$OS" == "Linux" ]]; then
    # Linux用: ALSA, v4l2デバイス
    echo "Audio devices (ALSA):"
    arecord -l 2>/dev/null | grep "^card" || echo "(not found)"
    echo "Video devices (v4l2):"
    v4l2-ctl --list-devices 2>/dev/null || echo "(not found)"
elif [[ "$OS" == "MINGW"* || "$OS" == "MSYS"* || "$OS" == "CYGWIN"* ]]; then
    # Windows(Git Bash等)用: DirectShowデバイス
    ffmpeg -list_devices true -f dshow -i dummy 2>&1 \
    | grep -E "DirectShow audio devices|DirectShow video devices|\s\""
    # 取得した行をそのまま出力するだけ
else
    # どの条件にも当てはまらない場合はエラー
    echo "未対応OSです: $OS"
    exit 1
fi

#!/usr/bin/env bash
# ffmpeg を使って単純な画面録画を行うスクリプト (macOS 用)
# Esc または q を押すと録画を停止し保存します
set -Eeuo pipefail

FFMPEG_BIN=${FFMPEG_BIN:-ffmpeg}
FPS=30
BITRATE=20M
SRC="1:none"
OUT_DIR="$HOME/Movies"

while getopts "f:b:s:o:h" o; do
  case $o in
    f) FPS=$OPTARG ;;
    b) BITRATE=$OPTARG ;;
    s) SRC=$OPTARG ;;
    o) OUT_DIR=$OPTARG ;;
    h|*) echo "usage: $0 [-f fps] [-b bitrate] [-s src] [-o out_dir]"; exit 0;;
  esac
done

mkdir -p "$OUT_DIR"
TS=$(date +%Y%m%d_%H%M%S)
OUT_FILE="$OUT_DIR/record_${TS}.mp4"

cleanup() {
  [[ -n "${FF_PID:-}" ]] && { kill -TERM "$FF_PID" 2>/dev/null || true; wait "$FF_PID" 2>/dev/null || true; }
  stty sane 2>/dev/null || true
  echo "done."
}
trap cleanup EXIT INT TERM

"$FFMPEG_BIN" -f avfoundation -pixel_format nv12 -framerate $((FPS*2)) -i "$SRC" \
  -vf "fps=$FPS,format=yuv420p" \
  -c:v h264_videotoolbox -realtime 1 -bf 0 -b:v "$BITRATE" \
  -g $((FPS*2)) -keyint_min $((FPS*2)) -sc_threshold 0 \
  -movflags +faststart "$OUT_FILE" & FF_PID=$!

echo "REC ▶︎  Esc/Q=quit"
stty -icanon -echo
while true; do
  if IFS= read -rsn1 k; then
    case "$k" in
      $'\x1b'|q) break ;;
    esac
  fi
done

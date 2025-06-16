#!/usr/bin/env bash
set -Eeuo pipefail

FFMPEG_BIN=${FFMPEG_BIN:-ffmpeg}
FPS=30 BITRATE=20M SRC="1:none" SEG_S=6 WRAP=11 RAM_MB=512 OUT_DIR="$HOME/Movies"

while getopts "f:b:s:t:n:r:o:h" o; do
  case $o in
    f) FPS=$OPTARG ;; b) BITRATE=$OPTARG ;;
    s) SRC=$OPTARG ;; t) SEG_S=$OPTARG ;;
    n) WRAP=$OPTARG ;; r) RAM_MB=$OPTARG ;;
    o) OUT_DIR=$OPTARG ;;
    h|*) echo "usage: $0 [-f fps] [-b bitrate] [-s src] [-t seg_sec] [-n wrap] [-r ram_mb] [-o out_dir]"; exit 0;;
  esac
done

GOP=$((FPS*SEG_S)) OFFSET=$((-(WRAP-1))) DUR=$((SEG_S*(WRAP-1)))
BLOCKS=$((RAM_MB*2048))
DEV=$(hdiutil attach -nomount "ram://$BLOCKS" | tr -d '[:space:]')
sudo diskutil erasevolume HFS+ RAMDisk "$DEV" >/dev/null
DIR=/Volumes/RAMDisk/replay; mkdir -p "$DIR"

# トリガーファイル
TRIGGER_SAVE="$DIR/.trigger_save"
TRIGGER_QUIT="$DIR/.trigger_quit"

cleanup() {
  [[ -n "${FF_PID:-}" ]] && { kill -TERM "$FF_PID" 2>/dev/null || true; sleep 1; kill -KILL "$FF_PID" 2>/dev/null || true; wait "$FF_PID" 2>/dev/null || true; }
  sudo diskutil eject /Volumes/RAMDisk >/dev/null || true
  hdiutil detach "$DEV" >/dev/null 2>&1 || true
  stty sane 2>/dev/null || true
  echo "done."
}
trap cleanup EXIT INT TERM

"$FFMPEG_BIN" -f avfoundation -pixel_format nv12 -framerate $((FPS*2)) -i "$SRC" \
  -vf "fps=$FPS,format=yuv420p" \
  -c:v h264_videotoolbox -realtime 1 -bf 0 -b:v "$BITRATE" \
  -g "$GOP" -keyint_min "$GOP" -sc_threshold 0 \
  -force_key_frames "expr:gte(t,n_forced*${SEG_S}-0.1)" -an \
  -f segment -segment_time "$SEG_S" -segment_format ts \
  -segment_wrap "$WRAP" -segment_list "$DIR/list.m3u8" \
  -segment_list_size "$WRAP" -segment_list_type m3u8 \
  -segment_list_flags +live "$DIR/seg%03d.ts" & FF_PID=$!

echo "REC ▶︎   s=save  Esc/Q=quit（ファイルトリガーも対応中！）"
stty -icanon -echo

while true; do
  k=""
  if IFS= read -rsn1 k; then
    : # キー入力あり
  fi

  # ファイルトリガー優先で上書き
  if [ -e "$TRIGGER_SAVE" ]; then
    k="s"
    rm -f "$TRIGGER_SAVE"
  fi
  if [ -e "$TRIGGER_QUIT" ]; then
    k="q"
    rm -f "$TRIGGER_QUIT"
  fi

  case "$k" in
    s)
      ts=$(date +%Y%m%d_%H%M%S)
      "$FFMPEG_BIN" -nostdin -y -live_start_index "$OFFSET" -i "$DIR/list.m3u8" \
             -t "$DUR" -c copy -movflags +faststart "$OUT_DIR/replay_$ts.mp4" 
      echo "📼 saved $ts"
      ;;
    $'\x1b'|q) break ;;
  esac
done

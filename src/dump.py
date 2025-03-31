import requests
import time
import json
import zipfile
import os
import signal
import sys
import argparse
from datetime import datetime
from threading import Event

# Live Client API のベースURLと対象エンドポイント
API_BASE = "https://127.0.0.1:2999/liveclientdata"
ENDPOINTS = [
    "allgamedata",
    "eventdata",
    "playerlist",
    "activeplayer",
    "activeplayerabilities"
]

# 出力先ディレクトリ（引数でファイル指定しないとき用）
DUMP_DIR = "debug_zips"
os.makedirs(DUMP_DIR, exist_ok=True)

# メモリ上のバッファと終了イベント
BUFFER = {ep: [] for ep in ENDPOINTS}
stop_event = Event()

def poll_once():
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S_%f")[:-3]
    game_ended = False
    for ep in ENDPOINTS:
        try:
            res = requests.get(f"{API_BASE}/{ep}", timeout=1.0)
            res.raise_for_status()
            data = res.json()
            BUFFER[ep].append((timestamp, data))
            if ep == "allgamedata":
                game_ended = data.get("gameData", {}).get("gameEnded", False)
        except Exception as e:
            print(f"[ERROR] {ep}: {e}")
    if game_ended:
        print("[INFO] ゲーム終了を検知しました。終了処理を行います。")
        stop_event.set()

def dump_zip(output_path=None):
    if all(len(entries) == 0 for entries in BUFFER.values()):
        print("[INFO] バッファ空なので保存スキップ")
        return

    if output_path is None:
        now = datetime.now().strftime("%Y%m%d_%H%M%S")
        output_path = os.path.join(DUMP_DIR, f"dump_{now}.zip")

    with zipfile.ZipFile(output_path, 'w', zipfile.ZIP_DEFLATED) as zipf:
        for ep, entries in BUFFER.items():
            for ts, data in entries:
                zipf.writestr(f"{ep}/{ts}.json", json.dumps(data, indent=2))
    print(f"[OK] ZIP保存完了: {output_path}")

    for ep in BUFFER:
        BUFFER[ep] = []

def handle_exit(output_path=None, *args):
    print("\n[INFO] 終了検知: 最後のZIP保存処理を開始します")
    dump_zip(output_path)
    stop_event.set()

def parse_args():
    parser = argparse.ArgumentParser(description="LoL LiveClientData ダンパ")
    parser.add_argument("output", nargs="?", help="保存先ZIPファイル名（省略時は自動）")
    return parser.parse_args()

def main_loop(output_path=None):
    start = time.time()
    try:
        while not stop_event.is_set():
            poll_once()
            time.sleep(1)
            if time.time() - start >= 10:
                dump_zip(output_path)
                start = time.time()
    finally:
        handle_exit(output_path)

if __name__ == "__main__":
    args = parse_args()
    signal.signal(signal.SIGINT, lambda s, f: handle_exit(args.output))
    signal.signal(signal.SIGTERM, lambda s, f: handle_exit(args.output))
    print("[INFO] LoLダンプ開始。Ctrl+Cで終了できます")
    main_loop(args.output)

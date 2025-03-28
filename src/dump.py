import requests
import os
import json
import time
import argparse
from datetime import datetime
from http.server import SimpleHTTPRequestHandler, HTTPServer
from threading import Thread

DUMP_DIR = "dump"
os.makedirs(DUMP_DIR, exist_ok=True)

def fetch_and_dump(endpoint: str, name: str) -> str:
    url = f"https://127.0.0.1:2999/liveclientdata/{endpoint}"
    try:
        res = requests.get(url, verify=False, timeout=2)
        res.raise_for_status()
        data = res.json()

        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        filename = f"{name}_{timestamp}.json"
        filepath = os.path.join(DUMP_DIR, filename)

        with open(filepath, "w", encoding="utf-8") as f:
            json.dump(data, f, ensure_ascii=False, indent=2)

        print(f"✅ {name} を {filepath} に保存したよ！")
        return filepath
    except Exception as e:
        print(f"❌ {name} の取得に失敗したよ: {e}")
        return ""

def run_mock_server(port: int):
    os.chdir(DUMP_DIR)
    handler = SimpleHTTPRequestHandler
    server = HTTPServer(("127.0.0.1", port), handler)
    print(f"🚀 モックサーバ起動中！http://127.0.0.1:{port}/")
    server.serve_forever()

def main():
    parser = argparse.ArgumentParser(description="LoL LiveClientDataのダンプツール")
    parser.add_argument("--interval", type=int, help="繰り返し取得する秒数間隔（省略時は1回だけ）")
    parser.add_argument("--mock", action="store_true", help="dumpディレクトリをモックサーバとして起動する")
    parser.add_argument("--mock-port", type=int, default=8080, help="モックサーバのポート番号（デフォルト: 8080）")
    args = parser.parse_args()

    if args.mock:
        thread = Thread(target=run_mock_server, args=(args.mock_port,), daemon=True)
        thread.start()

    try:
        while True:
            fetch_and_dump("allgamedata", "allgamedata")
            fetch_and_dump("eventdata", "eventdata")

            if args.interval:
                time.sleep(args.interval)
            else:
                break
    except KeyboardInterrupt:
        print("⏹️ 中断されたよ〜")

if __name__ == "__main__":
    main()

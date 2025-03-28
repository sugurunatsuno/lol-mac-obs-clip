import os
import json
import glob
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import urlparse
from threading import Lock

DUMP_DIR = "../dump"
MOCK_HOST = "127.0.0.1"
MOCK_PORT = 2999

# スレッドセーフなファイルインデックス管理用
class FileRotator:
    def __init__(self, pattern):
        self.files = sorted(glob.glob(pattern))
        self.index = 0
        self.lock = Lock()

    def next(self):
        with self.lock:
            if not self.files:
                return None
            file = self.files[self.index]
            self.index = (self.index + 1) % len(self.files)
            return file

# パスごとにローテーション用のファイル管理器を用意
rotators = {
    "/liveclientdata/allgamedata": FileRotator(os.path.join(DUMP_DIR, "allgamedata_*.json")),
    "/liveclientdata/eventdata": FileRotator(os.path.join(DUMP_DIR, "eventdata_*.json"))
}

class MockHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        parsed_path = urlparse(self.path)
        path = parsed_path.path

        if path in rotators:
            file = rotators[path].next()
            if file and os.path.exists(file):
                with open(file, "r", encoding="utf-8") as f:
                    data = f.read()
                self.send_response(200)
                self.send_header("Content-type", "application/json")
                self.end_headers()
                self.wfile.write(data.encode("utf-8"))
                print(f"📤 {path} に {file} を返したよ〜")
            else:
                self.send_error(404, "Data not found")
        else:
            self.send_error(404, "Unknown endpoint")

    def log_message(self, format, *args):
        return  # 標準ログを抑制（必要なら消してね）

def run_mock_server():
    httpd = HTTPServer((MOCK_HOST, MOCK_PORT), MockHandler)
    print(f"🚀 HTTPモックサーバ起動！http://{MOCK_HOST}:{MOCK_PORT}/")
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print("⏹️ モックサーバ停止されたよ〜")

if __name__ == "__main__":
    run_mock_server()

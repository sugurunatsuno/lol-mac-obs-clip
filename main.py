import asyncio
import threading
import tkinter as tk
from datetime import datetime, timedelta
import websockets
import json
import requests
import sys
import platform
import subprocess
import os

# ===== Live Client API endpoint =====
EVENT_API_URL = "http://127.0.0.1:2999/liveclientdata/eventdata"

# ===== GUIクラス =====
class EventGUI:
    def __init__(self):
        self.root = tk.Tk()
        self.root.title("LoL Event Logger")
        self.log_box = tk.Text(self.root, width=60, height=20)
        self.log_box.pack()
        self.root.protocol("WM_DELETE_WINDOW", self.on_close)

        # 半透明オーバーレイウィンドウを追加
        self.overlay = tk.Toplevel(self.root)
        self.overlay.title("Overlay")
        self.overlay.overrideredirect(True)
        self.overlay.geometry("100x100+100+100")
        self.overlay.attributes("-topmost", True)
        self.overlay.attributes("-alpha", 0.3)

        btn = tk.Button(self.overlay, text="Clip", command=self.on_overlay_click)
        btn.pack(expand=True, fill=tk.BOTH)

        # ドラッグ＆ドロップ対応
        btn.bind("<ButtonPress-1>", self.start_move)
        btn.bind("<B1-Motion>", self.do_move)

    def start_move(self, event):
        self._drag_start_x = event.x
        self._drag_start_y = event.y

    def do_move(self, event):
        x = self.overlay.winfo_x() + event.x - self._drag_start_x
        y = self.overlay.winfo_y() + event.y - self._drag_start_y
        self.overlay.geometry(f"+{x}+{y}")

    def on_overlay_click(self):
        self.log("🎯 オーバーレイボタンがクリックされました！")
        asyncio.run(trigger_replay_buffer())

    def log(self, message):
        now = datetime.now().strftime("%H:%M:%S")
        self.log_box.insert(tk.END, f"[{now}] {message}\n")
        self.log_box.see(tk.END)

    def run(self):
        self.root.mainloop()

    def on_close(self):
        print("👋 ウィンドウを閉じました。アプリを終了します。")
        stop_flag.set()
        self.root.destroy()
        sys.exit(0)

# ===== OBSのReplay Buffer保存コマンド =====
async def trigger_replay_buffer():
    try:
        async with websockets.connect("ws://localhost:4455") as ws:
            await ws.send(json.dumps({
                "op": 6,
                "d": {
                    "requestType": "SaveReplayBuffer",
                    "requestId": "save_clip",
                    "resource": "ReplayBuffer"
                }
            }))
            gui.log("🎬 Replay Buffer 保存リクエストを送信！")
            play_click_sound()
    except Exception as e:
        gui.log(f"❌ OBSエラー: {e}")

# ===== カシャ音を鳴らす処理 =====
def play_click_sound():
    sound_file = "shutter.wav"
    if not os.path.exists(sound_file):
        gui.log("⚠️ shutter.wav が見つかりません")
        return

    system = platform.system()
    try:
        if system == "Darwin":
            subprocess.run(["afplay", sound_file])
        elif system == "Windows":
            import winsound
            winsound.PlaySound(sound_file, winsound.SND_FILENAME)
        else:
            subprocess.run(["aplay", sound_file])
    except Exception as e:
        gui.log(f"⚠️ サウンド再生エラー: {e}")

# ===== マルチキル検知用ロジック =====
kill_log = []

last_event_time = None
seen_events = set()

def poll_lol_events():
    global last_event_time
    try:
        res = requests.get(EVENT_API_URL)
        data = res.json()
        events = data.get("Events", [])

        for e in events:
            event_id = (e.get("EventName"), e.get("EventTime"))
            if event_id in seen_events:
                continue
            seen_events.add(event_id)

            event_name = e.get("EventName")

            if event_name == "ChampionKill":
                now = datetime.now()
                kill_log.append(now)
                killer = e.get("KillerName", "Unknown")
                victim = e.get("VictimName", "Unknown")
                gui.log(f"💥 {killer} が {victim} をキル！")

                recent_kills = [t for t in kill_log if now - t < timedelta(seconds=10)]
                if len(recent_kills) >= 2:
                    gui.log(f"🔥 マルチキル（{len(recent_kills)}連続キル）→ 5秒後に保存予定")
                    threading.Timer(5.0, lambda: asyncio.run(trigger_replay_buffer())).start()
                    kill_log.clear()

            elif event_name == "Multikill":
                killer = e.get("KillerName", "Unknown")
                streak = e.get("KillStreak", 0)
                gui.log(f"🔥 {killer} が {streak}連続キルを達成！")

            elif event_name == "Ace":
                acer = e.get("Acer", "Unknown")
                team = e.get("AcingTeam", "Unknown")
                gui.log(f"🌟 ACE！{acer} による {team} チームの全滅！")

            elif event_name == "DragonKill":
                dragon = e.get("DragonType", "Unknown")
                killer = e.get("KillerName", "Unknown")
                gui.log(f"🐉 {killer} が {dragon} ドラゴンを討伐！")

            elif event_name == "BaronKill":
                killer = e.get("KillerName", "Unknown")
                gui.log(f"👑 {killer} がバロンを討伐！")

            elif event_name == "HeraldKill":
                killer = e.get("KillerName", "Unknown")
                gui.log(f"📯 {killer} がリフトヘラルドを討伐！")

            elif event_name == "TurretKilled":
                killer = e.get("KillerName", "Unknown")
                turret = e.get("TurretKilled", "Unknown")
                gui.log(f"🏰 {killer} がタワー({turret})を破壊！")

            elif event_name == "InhibKilled":
                killer = e.get("KillerName", "Unknown")
                inhib = e.get("InhibKilled", "Unknown")
                gui.log(f"💣 {killer} がインヒビター({inhib})を破壊！")

            elif event_name == "FirstBrick":
                killer = e.get("KillerName", "Unknown")
                gui.log(f"🧱 {killer} が最初のタワーを破壊！（ファーストブリック）")

            elif event_name == "GameStart":
                gui.log("🚩 ゲーム開始！")

            elif event_name == "MinionsSpawning":
                gui.log("🐾 ミニオンがスポーンしたよ！")

            else:
                gui.log(f"📌 イベント検出: {event_name}")

    except Exception as e:
        gui.log(f"⚠️ LoLイベント取得エラー: {e}")

# ===== イベントポーリングループ =====
stop_flag = threading.Event()

def event_loop():
    while not stop_flag.is_set():
        poll_lol_events()
        asyncio.run(asyncio.sleep(1.0))

# ===== 実行 =====
gui = EventGUI()
threading.Thread(target=event_loop, daemon=True).start()
gui.run()

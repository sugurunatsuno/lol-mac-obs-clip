import asyncio
import threading
import json
import requests
import sys
import platform
import subprocess
import os
from datetime import datetime, timedelta
import websockets
import logging
import tkinter as tk

# ===== ログ設定 =====
logger = logging.getLogger("LoL_Logger")
logger.setLevel(logging.DEBUG)
formatter = logging.Formatter('[%(asctime)s] %(levelname)s: %(message)s', datefmt='%H:%M:%S')

# コンソールハンドラー
console_handler = logging.StreamHandler(sys.stdout)
console_handler.setLevel(logging.DEBUG)
console_handler.setFormatter(formatter)

# ファイルハンドラー
file_handler = logging.FileHandler("event_log.txt", encoding="utf-8")
file_handler.setLevel(logging.DEBUG)
file_handler.setFormatter(formatter)

logger.addHandler(console_handler)
logger.addHandler(file_handler)

# ===== Live Client API endpoint =====
EVENT_API_URL = "https://127.0.0.1:2999/liveclientdata/eventdata"

# ===== グローバル変数：OBSとの持続的な接続 =====
obs_ws = None

async def get_obs_connection():
    global obs_ws
    # obs_wsがNoneか、接続が開いていなければ再接続するよ〜
    if obs_ws is None or not obs_ws.open:
        try:
            obs_ws = await websockets.connect("ws://localhost:4455")
            logger.info("OBSとの接続が確立されたよ〜！")
            # Identifyメッセージを送信（パスワードが不要の場合は空文字）
            identify_payload = json.dumps({
                "op": 1,
                "d": {
                    "rpcVersion": 1,
                    "authentication": ""
                }
            })
            await obs_ws.send(identify_payload)
            response = await obs_ws.recv()
            logger.info(f"OBSからのIdentifyレスポンス: {response}")
        except Exception as e:
            logger.error(f"❌ OBS接続またはIdentifyエラー: {e}")
            obs_ws = None
    return obs_ws

# ===== OBSのReplay Buffer保存コマンド（持続的な接続を利用） =====
async def trigger_replay_buffer():
    ws = await get_obs_connection()
    if ws is None:
        logger.error("❌ OBSへの接続が確立されていないよ〜")
        return
    try:
        await ws.send(json.dumps({
            "op": 6,
            "d": {
                "requestType": "SaveReplayBuffer",
                "requestId": "save_clip",
                "resource": "ReplayBuffer"
            }
        }))
        logger.info("🎬 Replay Buffer 保存リクエスト送信したよ〜！")
        play_click_sound()
    except Exception as e:
        logger.error(f"❌ OBS送信エラー: {e}")
        global obs_ws
        obs_ws = None  # エラー時は接続をリセットするよ〜

# ===== カシャ音を鳴らす処理 =====
def play_click_sound():
    sound_file = "shutter.mp3"
    if not os.path.exists(sound_file):
        logger.warning("⚠️ shutter.mp3 が見つからないよ〜")
        return

    try:
        # afplayの-vオプションで音量を0.3に設定（0.0〜1.0の範囲）
        subprocess.run(["afplay", "-v", "0.3", sound_file])
    except Exception as e:
        logger.warning(f"⚠️ サウンド再生エラー: {e}")


# ===== マルチキル検知用ロジック =====
kill_log = []
seen_events = set()

def poll_lol_events():
    try:
        # OpenAPI 3.0仕様に合わせAcceptヘッダーを追加！
        headers = {"Accept": "application/json"}
        params = {}  # 例: {"eventID": 0}  # 必要なら追加
        res = requests.get(EVENT_API_URL, headers=headers, params=params, timeout=1.0, verify=False)
        logger.info(f"🔍 LoLイベント取得中...（HTTP {res.status_code}）")
        data = res.json()
        events = data.get("Events", [])
        logger.info(f"🔍 LoLイベント取得中...（{len(events)}件）")
        
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
                logger.info(f"💥 {killer} が {victim} をキルしたよ〜！")
                recent_kills = [t for t in kill_log if now - t < timedelta(seconds=10)]
                if len(recent_kills) >= 2:
                    logger.info(f"🔥 マルチキル（{len(recent_kills)}連続キル）！5秒後に保存するよ〜")
                    threading.Timer(5.0, lambda: asyncio.run(trigger_replay_buffer())).start()
                    kill_log.clear()
            elif event_name == "Multikill":
                killer = e.get("KillerName", "Unknown")
                streak = e.get("KillStreak", 0)
                logger.info(f"🔥 {killer} が {streak}連続キル達成したよ〜！")
            elif event_name == "Ace":
                acer = e.get("Acer", "Unknown")
                team = e.get("AcingTeam", "Unknown")
                logger.info(f"🌟 ACE！{acer} による {team} チームの全滅だよ〜！")
            elif event_name == "DragonKill":
                dragon = e.get("DragonType", "Unknown")
                killer = e.get("KillerName", "Unknown")
                logger.info(f"🐉 {killer} が {dragon} ドラゴンを討伐したよ〜！")
            elif event_name == "BaronKill":
                killer = e.get("KillerName", "Unknown")
                logger.info(f"👑 {killer} がバロン討伐だよ〜！")
            elif event_name == "HeraldKill":
                killer = e.get("KillerName", "Unknown")
                logger.info(f"📯 {killer} がリフトヘラルド討伐したよ〜！")
            elif event_name == "TurretKilled":
                killer = e.get("KillerName", "Unknown")
                turret = e.get("TurretKilled", "Unknown")
                logger.info(f"🏰 {killer} がタワー({turret})を破壊したよ〜！")
            elif event_name == "InhibKilled":
                killer = e.get("KillerName", "Unknown")
                inhib = e.get("InhibKilled", "Unknown")
                logger.info(f"💣 {killer} がインヒビター({inhib})を破壊したよ〜！")
            elif event_name == "FirstBrick":
                killer = e.get("KillerName", "Unknown")
                logger.info(f"🧱 {killer} が最初のタワー破壊！（ファーストブリック）だよ〜！")
            elif event_name == "GameStart":
                logger.info("🚩 ゲーム開始だよ〜！")
            elif event_name == "MinionsSpawning":
                logger.info("🐾 ミニオンがスポーンしたよ〜！")
            else:
                logger.info(f"📌 イベント検出: {event_name}")
    except Exception as e:
        logger.error(f"⚠️ LoLイベント取得エラー: {e}")

def create_overlay_button():
    # メインウィンドウは非表示に
    root = tk.Tk()
    root.withdraw()

    # オーバーレイウィンドウを作成
    overlay = tk.Toplevel()
    overlay.overrideredirect(False)  # 枠を非表示に
    overlay.attributes("-topmost", True)  # 常に最前面に
    overlay.geometry("200x40+100+100")

    overlay.attributes("-alpha", 0.2)
    
    # Mac専用：ウィンドウがクリックしてもアクティブにならないように設定
    overlay.tk.call("::tk::unsupported::MacWindowStyle", "style", overlay._w, "help", "noActivates")
    
    # ボタン作成（ボタン自体もフォーカスを受け取らない）
    button = tk.Button(overlay, text="Clip", takefocus=0, command=lambda: print("Clip triggered!"))
    button.pack(expand=True, fill=tk.BOTH)

    # ドラッグ＆ドロップで移動できるように設定
    def start_move(event):
        overlay._drag_start_x = event.x
        overlay._drag_start_y = event.y

    def do_move(event):
        x = overlay.winfo_x() + event.x - overlay._drag_start_x
        y = overlay.winfo_y() + event.y - overlay._drag_start_y
        overlay.geometry(f"+{x}+{y}")

    button.bind("<ButtonPress-1>", start_move)
    button.bind("<B1-Motion>", do_move)

    overlay.mainloop()

# ===== イベントポーリングループ =====
stop_flag = threading.Event()

def event_loop():
    while not stop_flag.is_set():
        poll_lol_events()
        asyncio.run(asyncio.sleep(1.0))

# ===== main関数 =====
def main():
    logger.info("アプリケーション開始！")

    # オーバーレイボタンを作成
    create_overlay_button()

    # イベントポーリングスレッド開始
    event_thread = threading.Thread(target=event_loop, daemon=True)
    event_thread.start()
    
    # コマンドラインインターフェースで操作できるようにするよ〜
    logger.info("Enterキーでクリップ保存、'q'で終了。")
    try:
        while True:
            cmd = input("コマンド入力: ").strip().lower()
            if cmd == "q":
                break
            elif cmd == "":
                # Enterキーでクリップ保存
                asyncio.run(trigger_replay_buffer())
            else:
                logger.info("不明なコマンドだよ〜")
    except KeyboardInterrupt:
        logger.info("終了するよ〜")
    
    stop_flag.set()
    logger.info("アプリケーション終了。")

if __name__ == '__main__':
    main()

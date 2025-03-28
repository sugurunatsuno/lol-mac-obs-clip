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

from utils.option import Option, Some, None_
from utils.result import Result, Ok, Err

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
    if obs_ws is None or not obs_ws.open:
        try:
            obs_ws = await websockets.connect("ws://localhost:4455")
            logger.info("OBSとの接続が確立されたよ〜！")
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

# ===== OBSのReplay Buffer保存コマンド =====
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
        obs_ws = None

# ===== カシャ音を鳴らす処理 =====
def play_click_sound():
    sound_file = "shutter.mp3"
    if not os.path.exists(sound_file):
        logger.warning("⚠️ shutter.mp3 が見つからないよ〜")
        return
    try:
        subprocess.run(["afplay", "-v", "0.3", sound_file])
    except Exception as e:
        logger.warning(f"⚠️ サウンド再生エラー: {e}")

# ===== 設定ファイル読み込み処理 =====
config = {}
CONFIG_FILE = "config.json"

def load_config():
    global config
    try:
        with open(CONFIG_FILE, "r", encoding="utf-8") as f:
            config = json.load(f)
        logger.info("設定ファイルを読み込んだよ〜！")
    except Exception as e:
        logger.error(f"設定ファイルの読み込みに失敗したよ: {e}")
        config = {}

# ===== ActivePlayerの取得処理 =====
_active_player_cache: Option[str] = None_()
_active_player_timestamp: Option[datetime] = None_()

def get_active_player_name() -> Option[str]:
    global _active_player_cache, _active_player_timestamp

    now = datetime.now()
    if _active_player_cache.is_some() and _active_player_timestamp.is_some():
        if now - _active_player_timestamp.unwrap() < timedelta(minutes=1):
            return _active_player_cache  # ✅ キャッシュが有効！

    try:
        res = requests.get("https://127.0.0.1:2999/liveclientdata/activeplayer", verify=False, timeout=2)
        res.raise_for_status()
        data = res.json()
        summoner = data["summonerName"]
        _active_player_cache = Some(summoner)
        _active_player_timestamp = Some(now)
        return _active_player_cache
    except Exception as e:
        logger.warning(f"⚠️ ActivePlayerの取得に失敗したよ: {e}")
        return None_()



# ===== マルチキル検知用ロジック =====
kill_log = []
seen_events = set()

def poll_lol_events():
    try:
        headers = {"Accept": "application/json"}
        params = {}
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
            
            # ChampionKill イベント：マルチキル検出
            if event_name == "ChampionKill":
                now = datetime.now()
                kill_log.append(now)

                killer = e.get("KillerName", "Unknown")
                victim = e.get("VictimName", "Unknown")
                logger.info(f"💥 {killer} が {victim} をキルしたよ〜！")

                # ActivePlayer名と比較
                active_name = get_active_player_name()
                if active_name.is_some() and killer == active_name.unwrap():
                    recent_kills = [t for t in kill_log if now - t < timedelta(seconds=10)]
                    if len(recent_kills) >= 2:
                        if config.get("trigger_events", {}).get("ChampionKill", False):
                            delay = config.get("replay_delay", 5.0)
                            logger.info(f"🔥 {len(recent_kills)}連続キル！{delay}秒後に保存するよ〜")
                            threading.Timer(delay, lambda: asyncio.run(trigger_replay_buffer())).start()
                        kill_log.clear()


            elif event_name == "Multikill":
                killer = e.get("KillerName", "Unknown")
                streak = e.get("KillStreak", 0)
                logger.info(f"🔥 {killer} が {streak}連続キル達成したよ〜！")
                active_name = get_active_player_name()
                if active_name.is_some() and killer == active_name.unwrap():
                    if config.get("trigger_events", {}).get("Multikill", False):
                        delay = config.get("replay_delay", 5.0)
                        logger.info(f"🎬 自分のマルチキル！{delay}秒後に保存するよ〜")
                        threading.Timer(delay, lambda: asyncio.run(trigger_replay_buffer())).start()

            elif event_name == "Ace":
                acer = e.get("Acer", "Unknown")
                team = e.get("AcingTeam", "Unknown")
                logger.info(f"🌟 ACE！{acer} による {team} チームの全滅だよ〜！")
                if config.get("trigger_events", {}).get("Ace", False):
                    delay = config.get("replay_delay", 5.0)
                    logger.info(f"🎬 ACEイベントのため、{delay}秒後に保存するよ〜")
                    threading.Timer(delay, lambda: asyncio.run(trigger_replay_buffer())).start()

            elif event_name == "DragonKill":
                dragon = e.get("DragonType", "Unknown")
                killer = e.get("KillerName", "Unknown")
                logger.info(f"🐉 {killer} が {dragon} ドラゴンを討伐したよ〜！")
                if config.get("trigger_events", {}).get("DragonKill", False):
                    delay = config.get("replay_delay", 5.0)
                    logger.info(f"🎬 ドラゴン討伐イベントのため、{delay}秒後に保存するよ〜")
                    threading.Timer(delay, lambda: asyncio.run(trigger_replay_buffer())).start()

            elif event_name == "BaronKill":
                killer = e.get("KillerName", "Unknown")
                logger.info(f"👑 {killer} がバロン討伐だよ〜！")
                if config.get("trigger_events", {}).get("BaronKill", False):
                    delay = config.get("replay_delay", 5.0)
                    logger.info(f"🎬 バロン討伐イベントのため、{delay}秒後に保存するよ〜")
                    threading.Timer(delay, lambda: asyncio.run(trigger_replay_buffer())).start()

            elif event_name == "HeraldKill":
                killer = e.get("KillerName", "Unknown")
                logger.info(f"📯 {killer} がリフトヘラルド討伐したよ〜！")
                if config.get("trigger_events", {}).get("HeraldKill", False):
                    delay = config.get("replay_delay", 5.0)
                    logger.info(f"🎬 リフトヘラルド討伐イベントのため、{delay}秒後に保存するよ〜")
                    threading.Timer(delay, lambda: asyncio.run(trigger_replay_buffer())).start()

            elif event_name == "TurretKilled":
                killer = e.get("KillerName", "Unknown")
                turret = e.get("TurretKilled", "Unknown")
                logger.info(f"🏰 {killer} がタワー({turret})を破壊したよ〜！")
                if config.get("trigger_events", {}).get("TurretKilled", False):
                    delay = config.get("replay_delay", 5.0)
                    logger.info(f"🎬 タワー破壊イベントのため、{delay}秒後に保存するよ〜")
                    threading.Timer(delay, lambda: asyncio.run(trigger_replay_buffer())).start()

            elif event_name == "InhibKilled":
                killer = e.get("KillerName", "Unknown")
                inhib = e.get("InhibKilled", "Unknown")
                logger.info(f"💣 {killer} がインヒビター({inhib})を破壊したよ〜！")
                if config.get("trigger_events", {}).get("InhibKilled", False):
                    delay = config.get("replay_delay", 5.0)
                    logger.info(f"🎬 インヒビター破壊イベントのため、{delay}秒後に保存するよ〜")
                    threading.Timer(delay, lambda: asyncio.run(trigger_replay_buffer())).start()

            elif event_name == "FirstBrick":
                killer = e.get("KillerName", "Unknown")
                logger.info(f"🧱 {killer} が最初のタワー破壊！（ファーストブリック）だよ〜！")
                if config.get("trigger_events", {}).get("FirstBrick", False):
                    delay = config.get("replay_delay", 5.0)
                    logger.info(f"🎬 ファーストブリックイベントのため、{delay}秒後に保存するよ〜")
                    threading.Timer(delay, lambda: asyncio.run(trigger_replay_buffer())).start()

            elif event_name == "GameStart":
                logger.info("🚩 ゲーム開始だよ〜！")
                if config.get("trigger_events", {}).get("GameStart", False):
                    delay = config.get("replay_delay", 5.0)
                    logger.info(f"🎬 ゲーム開始イベントのため、{delay}秒後に保存するよ〜")
                    threading.Timer(delay, lambda: asyncio.run(trigger_replay_buffer())).start()

            elif event_name == "MinionsSpawning":
                logger.info("🐾 ミニオンがスポーンしたよ〜！")
                if config.get("trigger_events", {}).get("MinionsSpawning", False):
                    delay = config.get("replay_delay", 5.0)
                    logger.info(f"🎬 ミニオンスポーンイベントのため、{delay}秒後に保存するよ〜")
                    threading.Timer(delay, lambda: asyncio.run(trigger_replay_buffer())).start()

            else:
                logger.info(f"📌 イベント検出: {event_name}")

    except Exception as e:
        logger.error(f"⚠️ LoLイベント取得エラー: {e}")

def create_overlay_button():
    # メインウィンドウは非表示にするよ〜
    root = tk.Tk()
    root.withdraw()

    # オーバーレイウィンドウ作成
    overlay = tk.Toplevel()
    overlay.overrideredirect(False)
    overlay.attributes("-topmost", True)
    overlay.geometry("200x40+100+100")
    overlay.attributes("-alpha", 0.2)
    overlay.tk.call("::tk::unsupported::MacWindowStyle", "style", overlay._w, "help", "noActivates")
    
    # クリックしてもフォーカスを取得しないボタン作成
    button = tk.Button(overlay, text="Clip", takefocus=0, command=lambda: print("Clip triggered!"))
    button.pack(expand=True, fill=tk.BOTH)

    # ドラッグ＆ドロップでウィンドウ移動
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
    
    # 設定ファイルを読み込むよ〜
    load_config()
    
    # オーバーレイボタン作成
    create_overlay_button()

    # イベントポーリングスレッド開始
    event_thread = threading.Thread(target=event_loop, daemon=True)
    event_thread.start()
    
    logger.info("Enterキーでクリップ保存、'q'で終了。")
    try:
        while True:
            cmd = input("コマンド入力: ").strip().lower()
            if cmd == "q":
                break
            elif cmd == "":
                asyncio.run(trigger_replay_buffer())
            else:
                logger.info("不明なコマンドだよ〜")
    except KeyboardInterrupt:
        logger.info("終了するよ〜")
    
    stop_flag.set()
    logger.info("アプリケーション終了。")

if __name__ == '__main__':
    main()

import asyncio
import requests
from typing import Callable, Dict, List, Awaitable
from collections import deque
from utils.logger import logger
from utils.event_dispatcher import EventDispatcher
import urllib3
urllib3.disable_warnings(urllib3.exceptions.InsecureRequestWarning)


ALL_GAME_DATA_URL = "https://127.0.0.1:2999/liveclientdata/allgamedata"

def is_lol_client_running() -> bool:
    try:
        res = requests.get("https://127.0.0.1:2999/liveclientdata/gamestats", verify=False, timeout=1)
        return res.status_code == 200
    except:
        return False

# 体力変化の割合合計のしきい値（例: 40%）
HEALTH_CHANGE_THRESHOLD = 0.3
# ポーリング間隔（秒）
POLL_INTERVAL = 1
# 履歴の長さ
HISTORY_LENGTH = 10
class CustomEventPoller:
    def __init__(self, dispatcher: EventDispatcher):
        self._event_handlers: Dict[str, List[Callable[[dict], Awaitable[None]]]] = {}
        self._health_history: Dict[str, deque] = {}
        self._stop_event = asyncio.Event()
        self.dispatcher = dispatcher

    def register(self, event_name: str, handler: Callable[[dict], Awaitable[None]]) -> None:
        self._event_handlers.setdefault(event_name, []).append(handler)

    async def poll_events_async(self) -> None:
        last_client_state = None  # ← 最初は未確認状態としておく
        last_event_id = -1
        logger.info("🎯 LLEventPollerが起動したよ〜！")

        while not self._stop_event.is_set():
            is_running = is_lol_client_running()

            # 最初 or 状態変化時にログを出す！
            if is_running != last_client_state:
                if not is_running:
                    logger.debug("LoLクライアントが起動してないみたい、ちょっと待つね〜")
                else:
                    logger.info("LoLクライアントを見つけたよ！ポーリング再開するね〜")
                last_client_state = is_running

            if not is_running:
                await asyncio.sleep(1)
                continue

        while True:
            try:
                loop = asyncio.get_event_loop()
                response = await loop.run_in_executor(None, lambda: requests.get(ALL_GAME_DATA_URL, verify=False, timeout=2))
                response.raise_for_status()
                data = response.json()

                players = data.get("allPlayers", [])
                blue_deaths = 0
                red_deaths = 0
                total_health_change_ratio = 0.0

                for p in players:
                    name = p["summonerName"]
                    team = p["team"]
                    stats = p.get("championStats", {})
                    health = stats.get("currentHealth", 0.0)
                    max_health = stats.get("maxHealth", 1.0)

                    history = self._health_history.setdefault(name, deque(maxlen=HISTORY_LENGTH))
                    if history:
                        oldest = history[0]
                        total_health_change_ratio += abs(oldest - health) / max_health
                    history.append(health)

                    if health <= 0:
                        if team == "ORDER":
                            blue_deaths += 1
                        elif team == "CHAOS":
                            red_deaths += 1

                if (blue_deaths >= 2 and red_deaths >= 2) or total_health_change_ratio >= HEALTH_CHANGE_THRESHOLD:
                    logger.info("🔥 集団戦っぽい状況を検知したよ！")
                    await self.dispatcher.dispatch("teambattle", {})

            except Exception as e:
                logger.warning(f"Teamfightイベントポーリング失敗: {e}")

            await asyncio.sleep(POLL_INTERVAL)

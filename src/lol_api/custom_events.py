import asyncio
import requests
from typing import Callable, Dict, List, Awaitable
from utils.logger import logger

ALL_GAME_DATA_URL = "https://127.0.0.1:2999/liveclientdata/allgamedata"

_event_handlers: Dict[str, List[Callable[[dict], Awaitable[None]]]] = {}
_stop_event = asyncio.Event()
_previous_health: Dict[str, float] = {}

# 体力変化の割合合計のしきい値（例: 40%）
HEALTH_CHANGE_THRESHOLD = 0.3
# ポーリング間隔（秒）
POLL_INTERVAL = 10


def register_event_handler(event_name: str, handler: Callable[[dict], Awaitable[None]]) -> None:
    if event_name not in _event_handlers:
        _event_handlers[event_name] = []
    _event_handlers[event_name].append(handler)


async def poll_teamfight_events() -> None:
    while not _stop_event.is_set():
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
                prev = _previous_health.get(name, max_health)

                # 死亡カウント
                if health <= 0:
                    if team == "ORDER":
                        blue_deaths += 1
                    elif team == "CHAOS":
                        red_deaths += 1

                # HP変動率加算
                diff = abs(prev - health)
                total_health_change_ratio += diff / max_health

                # キャッシュ更新
                _previous_health[name] = health

            if (blue_deaths >= 2 and red_deaths >= 2) or total_health_change_ratio >= HEALTH_CHANGE_THRESHOLD:
                logger.info("🔥 集団戦っぽい状況を検知したよ！")
                for handler in _event_handlers.get("Teamfight", []):
                    try:
                        asyncio.create_task(handler({"type": "Teamfight"}))
                    except Exception as e:
                        logger.error(f"Teamfightイベント処理中にエラー発生: {e}")

        except Exception as e:
            logger.warning(f"Teamfightイベントポーリング失敗: {e}")

        await asyncio.sleep(POLL_INTERVAL)


def start_event_loop() -> None:
    asyncio.create_task(poll_teamfight_events())


def stop_event_loop() -> None:
    _stop_event.set()

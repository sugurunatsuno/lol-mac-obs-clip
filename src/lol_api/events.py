import requests
import asyncio
from typing import Callable, Dict, List, Awaitable
from utils.logger import logger
from utils.event_types import EventType  # 列挙型をインポート
from utils.event_dispatcher import EventDispatcher

EVENT_API_URL = "https://127.0.0.1:2999/liveclientdata/eventdata"

_event_handlers: Dict[EventType, List[Callable[[dict], Awaitable[None]]]] = {}
_stop_event = asyncio.Event()

def register_event_handler(event_type: EventType, handler: Callable[[dict], Awaitable[None]]) -> None:
    """
    指定したイベントタイプに対して非同期ハンドラ関数を登録します。

    Args:
        event_type (EventType): 登録するイベントの列挙型。
        handler (Callable[[dict], Awaitable[None]]): 非同期関数。
    """
    if event_type not in _event_handlers:
        _event_handlers[event_type] = []
    _event_handlers[event_type].append(handler)

async def poll_events_async() -> None:
    """
    LoLのイベントAPIを非同期でポーリングし、新しいイベントがあれば登録された非同期ハンドラを呼び出します。
    """
    last_event_id = -1

    while not _stop_event.is_set():
        try:
            loop = asyncio.get_event_loop()
            response = await loop.run_in_executor(None, lambda: requests.get(EVENT_API_URL, verify=False, timeout=2))
            response.raise_for_status()
            events_data = response.json()

            new_events = events_data.get("Events", [])

            for event in new_events:
                if event["EventID"] > last_event_id:
                    last_event_id = event["EventID"]
                    try:
                        event_type = EventType(event["EventName"])
                    except ValueError:
                        logger.debug(f"未定義のイベント: {event['EventName']}")
                        continue

                    handlers = _event_handlers.get(event_type, [])

                    for handler in handlers:
                        try:
                            asyncio.create_task(handler(event))
                        except Exception as e:
                            logger.error(f"イベント処理中にエラー発生: {e}")

        except Exception as e:
            logger.warning(f"イベントポーリング失敗: {e}")

        await asyncio.sleep(1)

def start_event_loop() -> None:
    """
    イベントのポーリングをバックグラウンドタスクで開始します。
    """
    asyncio.create_task(poll_events_async())

def stop_event_loop() -> None:
    """
    イベントのポーリングを停止します。
    """
    _stop_event.set()

def is_lol_client_running() -> bool:
    try:
        res = requests.get("https://127.0.0.1:2999/liveclientdata/gamestats", verify=False, timeout=1)
        return res.status_code == 200
    except:
        return False


class LLEventPoller:
    def __init__(self, dispatcher: EventDispatcher):
        self.dispatcher = dispatcher
        self._stop_event = asyncio.Event()

    async def poll_events_async(self) -> None:
        """
        LoLのイベントAPIをポーリングして、新しいイベントをdispatcherに渡すよ！
        """
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

            try:
                loop = asyncio.get_event_loop()
                response = await loop.run_in_executor(None, lambda: requests.get(EVENT_API_URL, verify=False, timeout=2))
                response.raise_for_status()
                events_data = response.json()
                new_events = events_data.get("Events", [])

                for event in new_events:
                    if event["EventID"] > last_event_id:
                        last_event_id = event["EventID"]

                        try:
                            event_type = EventType(event["EventName"])
                        except ValueError:
                            logger.debug(f"未定義のイベント: {event['EventName']} は無視するね〜")
                            continue

                        await self.dispatcher.dispatch(event_type, event)

            except Exception as e:
                logger.warning(f"⚠️ LoLイベントポーリング失敗: {e}")

            await asyncio.sleep(1)

    def stop(self):
        """
        ポーリングを止めるよ！
        """
        self._stop_event.set()
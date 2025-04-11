import asyncio
import requests
from utils.logger import fileonly_logger
from utils.event_dispatcher import EventDispatcher
from utils.event_types import CustomEventType
import urllib3
urllib3.disable_warnings(urllib3.exceptions.InsecureRequestWarning)

ALL_GAME_DATA_URL = "https://127.0.0.1:2999/liveclientdata/allgamedata"
POLL_INTERVAL = 1.0

class GameStatePoller:
    def __init__(self, dispatcher: EventDispatcher):
        self.dispatcher = dispatcher
        self._stop_event = asyncio.Event()

    async def poll_events_async(self):
        while not self._stop_event.is_set():
            try:
                loop = asyncio.get_event_loop()
                response = await loop.run_in_executor(
                    None,
                    lambda: requests.get(ALL_GAME_DATA_URL, verify=False, timeout=2)
                )
                response.raise_for_status()
                data = response.json()

                fileonly_logger.debug("🔎 ゲーム状態: %s", data)
                await self.dispatcher.dispatch(CustomEventType.GAME_STATE_UPDATE, data)

            except Exception as e:
                fileonly_logger.warning(f"[GameStatePoller] allgamedata取得失敗: {e}")
            await asyncio.sleep(POLL_INTERVAL)

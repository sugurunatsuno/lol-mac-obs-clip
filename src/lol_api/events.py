import requests
import time
import threading
from typing import Callable, Dict, List
from utils.logger import logger

EVENT_API_URL = "https://127.0.0.1:2999/liveclientdata/eventdata"

_event_handlers: Dict[str, List[Callable[[dict], None]]] = {}
_stop_event = threading.Event()


def register_event_handler(event_name: str, handler: Callable[[dict], None]) -> None:
    """
    指定したイベント名に対してハンドラ関数を登録します。

    Args:
        event_name (str): 登録するイベントの名前。
        handler (Callable[[dict], None]): イベントが発生したときに呼ばれる関数。
    """
    if event_name not in _event_handlers:
        _event_handlers[event_name] = []
    _event_handlers[event_name].append(handler)


def poll_events() -> None:
    """
    LoLのイベントAPIをポーリングし、新しいイベントがあれば登録されたハンドラを呼び出します。
    """
    last_event_id = -1

    while not _stop_event.is_set():
        try:
            response = requests.get(EVENT_API_URL, verify=False, timeout=2)
            response.raise_for_status()
            events_data = response.json()

            new_events = events_data.get("Events", [])

            for event in new_events:
                if event["EventID"] > last_event_id:
                    last_event_id = event["EventID"]
                    event_name = event["EventName"]
                    handlers = _event_handlers.get(event_name, [])

                    for handler in handlers:
                        try:
                            handler(event)
                        except Exception as e:
                            logger.error(f"イベント処理中にエラー発生: {e}")

        except Exception as e:
            logger.warning(f"イベントポーリング失敗: {e}")

        time.sleep(1)


def start_event_loop() -> None:
    """
    イベントのポーリングをバックグラウンドスレッドで開始します。
    """
    event_thread = threading.Thread(target=poll_events, daemon=True)
    event_thread.start()


def stop_event_loop() -> None:
    """
    イベントのポーリングを停止します。
    """
    _stop_event.set()

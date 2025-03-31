# utils/event_dispatcher.py

import asyncio
from typing import Callable, Awaitable, Dict, List, Union
from utils.logger import logger
from utils.event_types import EventType, CustomEventType

# 列挙型または文字列をサポート
EventKey = Union[EventType, CustomEventType, str]

class EventDispatcher:
    def __init__(self):
        self._handlers: Dict[str, List[Callable[[dict], Awaitable[None]]]] = {}

    def register(self, event_name: EventKey, handler: Callable[[dict], Awaitable[None]]) -> None:
        """
        指定したイベント名に非同期ハンドラを登録します。
        """
        name = str(event_name)
        if name not in self._handlers:
            self._handlers[name] = []
        self._handlers[name].append(handler)

    async def dispatch(self, event_name: EventKey, data: dict) -> None:
        """
        指定したイベント名に登録されている全てのハンドラを呼び出します。
        """
        name = str(event_name)
        handlers = self._handlers.get(name, [])
        for handler in handlers:
            try:
                asyncio.create_task(handler(data))
            except Exception as e:
                logger.error(f"イベント '{name}' のハンドラ処理中にエラー発生: {e}")

# tests/test_poller.py

import asyncio
import pytest
from unittest.mock import AsyncMock, patch, MagicMock

from lol_api.events import LLEventPoller
from utils.event_dispatcher import EventDispatcher
from utils.event_types import EventType

@pytest.mark.asyncio
async def test_poller_dispatches_event_once():
    # モックdispatcher
    dispatcher = MagicMock(spec=EventDispatcher)
    dispatcher.dispatch = AsyncMock()

    # テスト対象
    poller = LLEventPoller(dispatcher)

    fake_event_data = {
        "Events": [
            {
                "EventID": 1,
                "EventName": "ChampionKill",
                "KillerName": "Akari",
            }
        ]
    }

    with patch("lol_api.events.requests.get") as mock_get:
        # モックレスポンス
        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_response.json.return_value = fake_event_data
        mock_get.return_value = mock_response

        # ストップイベントを1回のループ後にトリガーする
        async def stop_after():
            await asyncio.sleep(0.1)
            poller.stop()

        # 並列でポーラー実行
        await asyncio.gather(
            poller.poll_events_async(),
            stop_after()
        )

    # dispatcher.dispatchが1回呼ばれたことを確認
    dispatcher.dispatch.assert_awaited_once_with(EventType.CHAMPION_KILL, fake_event_data["Events"][0])

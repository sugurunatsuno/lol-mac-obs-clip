# src/tests/test_main.py

import pytest
import asyncio
from unittest.mock import patch, AsyncMock, MagicMock
from main import trigger_replay, dispatcher, CONFIG
from utils.option import Some, None_

@patch("main.get_active_player_name", return_value=Some("Akari"))
@patch("main.trigger_replay_buffer", new_callable=AsyncMock)
@pytest.mark.asyncio
async def test_trigger_replay_calls_obs(mock_trigger, mock_player):
    event = {"KillerName": "Akari"}
    await trigger_replay(event, delay=0.01, message="test message")
    mock_trigger.assert_awaited_once()

@patch("main.get_active_player_name", return_value=Some("NotAkari"))
@patch("main.trigger_replay_buffer", new_callable=AsyncMock)
@pytest.mark.asyncio
async def test_trigger_replay_not_matching_player(mock_trigger, mock_player):
    event = {"KillerName": "SomeoneElse"}
    await trigger_replay(event, delay=0.01, message="test message")
    mock_trigger.assert_not_awaited()

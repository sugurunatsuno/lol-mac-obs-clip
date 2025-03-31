# src/tests/test_player.py

import pytest
from unittest.mock import patch, MagicMock
from lol_api.player import get_active_player_name, _active_player_cache, _active_player_timestamp
from utils.option import Some, None_
from datetime import datetime, timedelta

def test_get_active_player_name_cache_hit():
    # キャッシュ有効
    now = datetime.now()
    _active_player_cache._value = "Akari"
    _active_player_timestamp._value = now

    result = get_active_player_name()
    assert result.is_some()
    assert result.unwrap() == "Akari"

@patch("lol_api.player.requests.get")
def test_get_active_player_name_api_success(mock_get):
    # キャッシュ無効・API成功
    _active_player_cache._value = None
    _active_player_timestamp._value = None

    mock_res = MagicMock()
    mock_res.json.return_value = {"summonerName": "Kokage"}
    mock_res.status_code = 200
    mock_res.raise_for_status = MagicMock()
    mock_get.return_value = mock_res

    result = get_active_player_name()
    assert result.is_some()
    assert result.unwrap() == "Kokage"

@patch("lol_api.player.requests.get", side_effect=Exception("No connection"))
def test_get_active_player_name_api_failure(mock_get):
    # API失敗
    _active_player_cache._value = None
    _active_player_timestamp._value = None

    result = get_active_player_name()
    assert result.is_none()

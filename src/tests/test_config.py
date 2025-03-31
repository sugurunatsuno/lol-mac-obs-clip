import pytest
import json
import os
from unittest.mock import patch, mock_open
from utils.config import load_config, create_default_config, CONFIG_FILE

def test_load_config_file_not_found():
    with patch("os.path.exists", return_value=False):
        config = load_config()
    assert config == {}

@patch("builtins.open", new_callable=mock_open, read_data='{"trigger_events": {"ChampionKill": true}}')
@patch("os.path.exists", return_value=True)
def test_load_config_success(mock_exists, mock_file):
    config = load_config()
    assert config["trigger_events"]["ChampionKill"] is True

@patch("builtins.open", new_callable=mock_open, read_data='INVALID_JSON')
@patch("os.path.exists", return_value=True)
def test_load_config_invalid_json(mock_exists, mock_file):
    config = load_config()
    assert config == {}

@patch("builtins.open", new_callable=mock_open)
def test_create_default_config(mock_file):
    with patch("json.dump") as mock_dump:
        create_default_config()
        mock_file.assert_called_once_with(CONFIG_FILE, "w", encoding="utf-8")
        args, kwargs = mock_dump.call_args
        assert "trigger_events" in args[0]

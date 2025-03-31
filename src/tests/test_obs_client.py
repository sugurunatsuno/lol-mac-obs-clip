import pytest
from unittest.mock import AsyncMock, patch
from obs.obs_client import get_obs_connection, trigger_replay_buffer
from utils.result import Result


@pytest.mark.asyncio
async def test_get_obs_connection_success():
    fake_ws = AsyncMock()
    fake_ws.open = True

    async def mock_recv():
        return '{"op":2,"d":{"rpcVersion":1}}'

    fake_ws.recv = mock_recv

    with patch("obs.obs_client.websockets.connect", return_value=fake_ws):
        result = await get_obs_connection()

    assert result.is_ok()
    assert result.unwrap() == fake_ws


@pytest.mark.asyncio
async def test_get_obs_connection_failure():
    with patch("obs.obs_client.websockets.connect", side_effect=Exception("Connection failed")):
        result = await get_obs_connection()
    assert result.is_err()
    assert "Connection failed" in result.unwrap_err()


@pytest.mark.asyncio
async def test_trigger_replay_buffer_success():
    fake_ws = AsyncMock()
    fake_ws.open = True
    fake_ws.recv = AsyncMock(side_effect=[
        '{"op":2,"d":{"rpcVersion":1}}',  # Identify response
        '{"op":7,"d":{"requestId":"saveReplay","requestType":"SaveReplayBuffer"}}'  # SaveReplayBuffer response
    ])

    with patch("obs.obs_client.websockets.connect", return_value=fake_ws):
        result = await trigger_replay_buffer()

    assert result.is_ok()
    fake_ws.send.assert_any_call(
        '{"op": 6, "d": {"requestType": "SaveReplayBuffer", "requestId": "saveReplay"}}'
    )


@pytest.mark.asyncio
async def test_trigger_replay_buffer_failure():
    fake_ws = AsyncMock()
    fake_ws.open = True
    fake_ws.recv = AsyncMock(side_effect=Exception("Send failed"))

    with patch("obs.obs_client.websockets.connect", return_value=fake_ws):
        result = await trigger_replay_buffer()

    assert result.is_err()
    assert "Send failed" in result.unwrap_err()

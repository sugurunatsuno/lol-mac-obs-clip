import pytest
import zipfile
import tempfile
import shutil
import os
import json
from unittest.mock import patch
from dump import poll_once, dump_zip, BUFFER

@pytest.fixture
def temp_output_dir():
    path = tempfile.mkdtemp()
    yield path
    shutil.rmtree(path)

@patch("dump.requests.get")
def test_poll_once_collects_data(mock_get):
    # モックレスポンスを設定
    def mock_response(url, timeout):
        class MockResp:
            def raise_for_status(self): pass
            def json(self):
                if "allgamedata" in url:
                    return {"gameData": {"gameEnded": False}}
                return {"data": "test"}
        return MockResp()
    mock_get.side_effect = mock_response

    # バッファクリア
    for ep in BUFFER:
        BUFFER[ep] = []

    poll_once()

    # 各エンドポイントにデータが1件追加されたか確認
    for ep in BUFFER:
        assert len(BUFFER[ep]) == 1
        ts, data = BUFFER[ep][0]
        assert isinstance(ts, str)
        assert isinstance(data, dict)

@patch("dump.BUFFER", {ep: [("20250101_000000_000", {"foo": "bar"})] for ep in [
    "allgamedata", "eventdata", "playerlist", "activeplayer", "activeplayerabilities"
]})
def test_dump_zip_creates_valid_zip(temp_output_dir):
    zip_path = os.path.join(temp_output_dir, "test_output.zip")
    dump_zip(zip_path)

    assert os.path.exists(zip_path)
    with zipfile.ZipFile(zip_path, 'r') as zipf:
        names = zipf.namelist()
        # すべてのエンドポイント分のファイルがあるか
        for ep in BUFFER.keys():
            expected = f"{ep}/20250101_000000_000.json"
            assert expected in names
            with zipf.open(expected) as f:
                content = json.load(f)
                assert content["foo"] == "bar"

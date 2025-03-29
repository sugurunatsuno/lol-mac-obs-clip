import requests
from datetime import datetime, timedelta
from utils.option import Option, Some, None_
from utils.logger import logger

_active_player_cache: Option[str] = None_()
_active_player_timestamp: Option[datetime] = None_()

ACTIVE_PLAYER_URL = "https://127.0.0.1:2999/liveclientdata/activeplayer"

def get_active_player_name() -> Option[str]:
    """

    :rtype: object
    """
    global _active_player_cache, _active_player_timestamp

    now = datetime.now()
    if _active_player_cache.is_some() and _active_player_timestamp.is_some():
        if now - _active_player_timestamp.unwrap() < timedelta(minutes=1):
            return _active_player_cache

    try:
        res = requests.get(ACTIVE_PLAYER_URL, verify=False, timeout=2)
        res.raise_for_status()
        data = res.json()
        summoner = data["summonerName"]
        _active_player_cache = Some(summoner)
        _active_player_timestamp = Some(now)
        return _active_player_cache
    except Exception as e:
        logger.warning(f"⚠️ ActivePlayerの取得に失敗したよ: {e}")
        return None_()

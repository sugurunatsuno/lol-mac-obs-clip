import asyncio
import threading
from utils.logger import logger
from utils.config import load_config, create_default_config
from utils.event_dispatcher import EventDispatcher
from utils.event_types import EventType, CustomEventType
from obs.obs_client import trigger_replay_buffer
from lol_api.player import get_active_player_name
from lol_api.events import LLEventPoller
from lol_api.custom_events import CustomEventPoller

CONFIG = {}
dispatcher = EventDispatcher()

async def trigger_replay(event: dict, delay: float, message: str):
    """
    リプレイを保存するトリガーを発火させる関数
    :param event: イベントデータ
    :param delay: リプレイ保存までの遅延時間
    :param message: リプレイ保存のメッセージ
    :return: None
    """
    active_player = get_active_player_name()
    if active_player.is_some() and event.get("KillerName") == active_player.unwrap():
        logger.info(f"💥 {message} {delay}秒後にリプレイを保存するね〜")
        await asyncio.sleep(delay)
        await trigger_replay_buffer()

def make_replay_handler(message: str):
    """
    リプレイ保存のハンドラを作成する関数
    :param message: リプレイ保存のメッセージ
    :return: 非同期ハンドラ関数
    """
    async def handler(event: dict):
        """
        リプレイ保存のハンドラ
        :param event: イベントデータ
        :return: None
        """
        await trigger_replay(event, CONFIG.get("replay_delay", 5.0), message)
    return handler

handle_champion_kill = make_replay_handler("自分がキルしたよ！")
handle_multikill = make_replay_handler("自分がマルチキルしたよ！")
handle_player_death = make_replay_handler("自分がデスしたよ！")
handle_dragon_steal = make_replay_handler("ドラゴンを盗んだよ！")
handle_grabs_steal = make_replay_handler("グラブを盗んだよ！")
handle_herald_steal = make_replay_handler("ヘラルドを盗んだよ！")
handle_baron_steal = make_replay_handler("バロンを盗んだよ！")
handle_ace = make_replay_handler("自分がエースしたよ！")
handle_teambattle = make_replay_handler("集団戦が起きたよ！")

async def main_async():
    global CONFIG

    CONFIG = load_config()
    if not CONFIG:
        create_default_config()
        CONFIG = load_config()

    trigger_events = CONFIG.get("trigger_events", {})
    handlers = {
        "ChampionKill": (EventType.CHAMPION_KILL, handle_champion_kill),
        "Multikill": (EventType.MULTIKILL, handle_multikill),
        "PlayerDeath": (EventType.PLAYER_DEATH, handle_player_death),
        "DragonSteal": (EventType.DRAGON_STEAL, handle_dragon_steal),
        "GrabsSteal": (EventType.GRABS_STEAL, handle_grabs_steal),
        "HeraldSteal": (EventType.HERALD_STEAL, handle_herald_steal),
        "BaronSteal": (EventType.BARON_STEAL, handle_baron_steal),
        "Ace": (EventType.ACE, handle_ace),

        # カスタムイベント
        "Teambattle": (CustomEventType.TEAM_FIGHT, handle_teambattle),
    }

    for event_name, (event_type, handler) in handlers.items():
        if trigger_events.get(event_name, False):
            dispatcher.register(event_type, handler)
            logger.info(f"イベント '{event_name}' にハンドラを登録したよ〜")

    poller = LLEventPoller(dispatcher)
    custom_poller = CustomEventPoller(dispatcher)
    asyncio.create_task(poller.poll_events_async())
    asyncio.create_task(custom_poller.poll_events_async())

    logger.info("LoL OBS Replay Trigger が起動したよ〜！終了するには Ctrl+C を押してね〜")

    try:
        await asyncio.Event().wait()
    except KeyboardInterrupt:
        logger.info("終了するよ〜")

if __name__ == "__main__":
    asyncio.run(main_async())
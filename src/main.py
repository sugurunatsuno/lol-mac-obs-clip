import asyncio
import threading
from utils.logger import logger
from utils.config import load_config, create_default_config
from utils.event_dispatcher import EventDispatcher
from utils.event_types import EventType, CustomEventType
from obs.obs_client import trigger_replay_buffer
from lol_api.player import get_active_player_name
from lol_api.events import LLEventPoller

CONFIG = {}
dispatcher = EventDispatcher()

async def trigger_replay(event: dict, delay: float, message: str):
    """

    :param event:
    :param delay:
    :param message:
    :return:
    """
    active_player = get_active_player_name()
    if active_player.is_some() and event.get("KillerName") == active_player.unwrap():
        logger.info(f"💥 {message} {delay}秒後にリプレイを保存するね〜")
        await asyncio.sleep(delay)
        await trigger_replay_buffer()

async def handle_champion_kill(event: dict):
    await trigger_replay(event, CONFIG.get("replay_delay", 5.0), "自分がキルしたよ！")

async def handle_multikill(event: dict):
    await trigger_replay(event, CONFIG.get("replay_delay", 5.0), "自分がマルチキルしたよ！")

async def handle_player_death(event: dict):
    await trigger_replay(event, CONFIG.get("replay_delay", 5.0), "自分がデスしたよ！")

async def handle_dragon_steal(event: dict):
    await trigger_replay(event, CONFIG.get("replay_delay", 5.0), "")

async def handle_grabs_steal(event: dict):
    await trigger_replay(event, CONFIG.get("replay_delay", 5.0), "")

async def handle_herald_steal(event: dict):
    await trigger_replay(event, CONFIG.get("replay_delay", 5.0), "")

async def handle_baron_steal(event: dict):
    await trigger_replay(event, CONFIG.get("replay_delay", 5.0), "")

async def handle_ace(event: dict):
    await trigger_replay(event, CONFIG.get("replay_delay", 5.0), "")

async def handle_teambattle(event: dict):
    await trigger_replay(event, CONFIG.get("replay_delay", 5.0), "")

async def main_async():
    global CONFIG

    CONFIG = load_config()
    if not CONFIG:
        create_default_config()
        CONFIG = load_config()

    if CONFIG.get("trigger_events", {}).get("ChampionKill", False):
        dispatcher.register(EventType.CHAMPION_KILL, handle_champion_kill)

    if CONFIG.get("trigger_events", {}).get("Multikill", False):
        dispatcher.register(EventType.MULTIKILL, handle_multikill)

    poller = LLEventPoller(dispatcher)
    asyncio.create_task(poller.poll_events_async())

    logger.info("LoL OBS Replay Trigger が起動したよ〜！終了するには Ctrl+C を押してね〜")

    try:
        await asyncio.Event().wait()
    except KeyboardInterrupt:
        logger.info("終了するよ〜")

if __name__ == "__main__":
    asyncio.run(main_async())
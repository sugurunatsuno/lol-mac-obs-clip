import asyncio
import threading
from utils.logger import logger
from utils.config import load_config, create_default_config
from obs.obs_client import trigger_replay_buffer
from lol_api.events import register_event_handler, start_event_loop, stop_event_loop
from lol_api.player import get_active_player_name

CONFIG = {}

def handle_champion_kill(event: dict):
    active_player = get_active_player_name()
    if active_player.is_some() and event.get("KillerName") == active_player.unwrap():
        delay = CONFIG.get("replay_delay", 5.0)
        logger.info(f"🔥 自分がチャンピオンを倒したよ！{delay}秒後にリプレイを保存するね〜")
        threading.Timer(delay, lambda: asyncio.run(trigger_replay_buffer())).start()

def handle_multikill(event: dict):
    active_player = get_active_player_name()
    if active_player.is_some() and event.get("KillerName") == active_player.unwrap():
        delay = CONFIG.get("replay_delay", 5.0)
        logger.info(f"🎬 自分がマルチキルしたよ！{delay}秒後にリプレイを保存するね〜")
        threading.Timer(delay, lambda: asyncio.run(trigger_replay_buffer())).start()

def main():
    global CONFIG

    CONFIG = load_config()
    if not CONFIG:
        create_default_config()
        CONFIG = load_config()

    if CONFIG.get("trigger_events", {}).get("ChampionKill", False):
        register_event_handler("ChampionKill", handle_champion_kill)

    if CONFIG.get("trigger_events", {}).get("Multikill", False):
        register_event_handler("Multikill", handle_multikill)

    start_event_loop()

    logger.info("LoL OBS Replay Trigger が起動したよ〜！終了するには Ctrl+C を押してね〜")

    try:
        while True:
            pass
    except KeyboardInterrupt:
        logger.info("終了するよ〜")
        stop_event_loop()

if __name__ == "__main__":
    main()

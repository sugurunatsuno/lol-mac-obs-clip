import asyncio
import threading
from utils.logger import logger
from utils.config import load_config, create_default_config
from obs.obs_client import trigger_replay_buffer
from lol_api.events import register_event_handler, start_event_loop, stop_event_loop
from lol_api.player import get_active_player_name

CONFIG = {}

async def trigger_replay(event: dict, delay: float, message: str):
    """
    リプレイ保存をトリガーする非同期ハンドラ。

    Args:
        event (dict): イベントデータ。
    """
    active_player = get_active_player_name()
    if active_player.is_some() and event.get("KillerName") == active_player.unwrap():
        logger.info(f"💥 {message} {delay}秒後にリプレイを保存するね〜")
        await asyncio.sleep(delay)
        await trigger_replay_buffer()

async def handle_champion_kill(event: dict):
    """
    チャンピオンキルイベント時にリプレイ保存をトリガーする非同期ハンドラ。

    Args:
        event (dict): イベントデータ。
    """
    await trigger_replay(event, CONFIG.get("replay_delay", 5.0), "自分がキルしたよ！")

async def handle_multikill(event: dict):
    """
    マルチキルイベント時にリプレイ保存をトリガーする非同期ハンドラ。

    Args:
        event (dict): イベントデータ。
    """
    await trigger_replay(event, CONFIG.get("replay_delay", 5.0), "自分がマルチキルしたよ！")

async def handle_player_death(event: dict):
    """
    プレイヤー死亡イベント時にリプレイ保存をトリガーする非同期ハンドラ。

    Args:
        event (dict): イベントデータ。
    """
    await trigger_replay(event, CONFIG.get("replay_delay", 5.0), "自分がデスしたよ！")

async def handle_dragon_steal(event: dict):
    """
    ドラゴン奪取イベント時にリプレイ保存をトリガーする非同期ハンドラ。

    Args:
        event (dict): イベントデータ。
    """
    await trigger_replay(event, CONFIG.get("replay_delay", 5.0), "")

async def handle_grabs_steal(event: dict):
    """
    グラブ奪取イベント時にリプレイ保存をトリガーする非同期ハンドラ。

    Args:
        event (dict): イベントデータ。
    """
    await trigger_replay(event, CONFIG.get("replay_delay", 5.0), "")


async def handle_herald_steal(event: dict):
    """
    ヘラルド奪取イベント時にリプレイ保存をトリガーする非同期ハンドラ。

    Args:
        event (dict): イベントデータ。
    """
    await trigger_replay(event, CONFIG.get("replay_delay", 5.0), "")

async def handle_baron_steal(event: dict):
    """
    バロン奪取イベント時にリプレイ保存をトリガーする非同期ハンドラ。

    Args:
        event (dict): イベントデータ。
    """
    await trigger_replay(event, CONFIG.get("replay_delay", 5.0), "")

async def handle_ace(event: dict):
    """
    エースイベント時にリプレイ保存をトリガーする非同期ハンドラ。

    Args:
        event (dict): イベントデータ。
    """
    await trigger_replay(event, CONFIG.get("replay_delay", 5.0), "")


async def handle_teambattle(event: dict):
    """
    チームバトルイベント時にリプレイ保存をトリガーする非同期ハンドラ。
    チームバトルはイベントとしては存在しないため、独自に作成しています
    Args:
        event (dict): イベントデータ。
    """
    # 1. 一定時間内に各チームのチャンピオンが2体以上キルされた場合が条件
    # 2. その場合、リプレイ保存をトリガーする

    await trigger_replay(event, CONFIG.get("replay_delay", 5.0), "")



async def main_async():
    """
    メインの非同期処理。設定を読み込み、イベントハンドラを登録してイベントループを開始します。
    """
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
        await asyncio.Event().wait()  # 永久待機
    except KeyboardInterrupt:
        logger.info("終了するよ〜")
        stop_event_loop()

if __name__ == "__main__":
    asyncio.run(main_async())

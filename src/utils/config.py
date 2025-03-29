import json
import os
from utils.logger import logger

CONFIG_FILE = "config.json"

def load_config() -> dict:
    if not os.path.exists(CONFIG_FILE):
        logger.warning(f"設定ファイルが見つからないよ〜: {CONFIG_FILE}")
        return {}

    try:
        with open(CONFIG_FILE, "r", encoding="utf-8") as f:
            config = json.load(f)
        logger.info("設定ファイルを読み込んだよ〜")
        return config
    except Exception as e:
        logger.error(f"設定ファイルの読み込みに失敗したよ〜: {e}")
        return {}

def create_default_config() -> None:
    default_config = {
        "trigger_events": {
            "ChampionKill": True,
            "Multikill": True
        },
        "replay_delay": 5.0
    }

    try:
        with open(CONFIG_FILE, "w", encoding="utf-8") as f:
            json.dump(default_config, f, ensure_ascii=False, indent=4)
        logger.info("デフォルトの設定ファイルを作成したよ〜")
    except Exception as e:
        logger.error(f"デフォルト設定ファイルの作成に失敗したよ〜: {e}")

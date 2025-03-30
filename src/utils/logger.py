import logging
import sys

logger = logging.getLogger("lol-replay")
logger.setLevel(logging.DEBUG)

# コンソール用（80文字制限）
console_handler = logging.StreamHandler(sys.stdout)
console_handler.setLevel(logging.INFO)
console_formatter = logging.Formatter("[%(levelname).1s] %(message).80s")
console_handler.setFormatter(console_formatter)

# ファイル用（詳細ログ）
file_handler = logging.FileHandler("replay_trigger.log", encoding="utf-8")
file_handler.setLevel(logging.DEBUG)
file_formatter = logging.Formatter(
    "%(asctime)s [%(levelname)s] %(name)s: %(message)s"
)
file_handler.setFormatter(file_formatter)

# 追加
logger.addHandler(console_handler)
logger.addHandler(file_handler)

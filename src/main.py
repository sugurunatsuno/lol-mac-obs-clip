import asyncio
import aiohttp
import websockets
import json
import logging

ALLGAMEDATA_URL = "https://127.0.0.1:2999/liveclientdata/allgamedata"
OBS_WS_URL = "ws://localhost:4455"
HTTP_TIMEOUT = aiohttp.ClientTimeout(total=2)

# ---- ロガーセットアップ ----
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    handlers=[
        logging.StreamHandler(),
        logging.FileHandler("lol_obs_replay.log", encoding="utf-8")
    ]
)
logger = logging.getLogger("lol_obs_replay")

# ---- カスタムイベント管理 ----
class CustomEventManager:
    def __init__(self):
        self._handlers = {}

    def register(self, event_name, handler):
        self._handlers[event_name] = handler

    async def check_all(self, history, my_name):
        # 集団戦（死亡2人以上）
        if self.detect_teamfight(history):
            if "TeamFight" in self._handlers:
                await self._handlers["TeamFight"]({"history": history})

        # 通常イベント（履歴の最新から）
        last = history[-1] if history else {}
        for event in last.get("events", {}).get("Events", []):
            # マルチキル
            if event.get("EventName") == "Multikill" and event.get("KillerName") == my_name:
                if "MyMultikill" in self._handlers:
                    await self._handlers["MyMultikill"](event)
            # デス
            if event.get("EventName") == "ChampionDeath" and event.get("VictimName") == my_name:
                if "MyDeath" in self._handlers:
                    await self._handlers["MyDeath"](event)

    def detect_teamfight(self, history):
        # 集団戦検出ロジック（死亡者2人以上）
        if not history:
            return False
        last = history[-1]
        deaths = sum(1 for p in last.get("allPlayers", []) if p.get("isDead"))
        return deaths >= 2

# ---- OBSトリガー ----
async def trigger_obs_replay():
    try:
        async with websockets.connect(OBS_WS_URL) as ws:
            payload = {
                "op": 6,
                "d": {
                    "requestType": "SaveReplayBuffer",
                    "requestId": "saveReplay"
                }
            }
            await ws.send(json.dumps(payload))
            resp = await ws.recv()
            logger.info("Replay triggered! OBS response: %s", resp)
    except Exception as e:
        logger.error("OBS連携失敗: %s", e)

# ---- サモナーネーム自動取得 ----
async def get_summoner_name(session: aiohttp.ClientSession):
    logger.info("サモナーネーム自動取得: ゲーム開始待機中…")
    while True:
        try:
            async with session.get(ALLGAMEDATA_URL, timeout=HTTP_TIMEOUT) as resp:
                data = await resp.json()
            if "activePlayer" in data and "summonerName" in data["activePlayer"]:
                name = data["activePlayer"]["summonerName"]
                logger.info("ゲーム開始検知！自分のサモナーネーム: %s", name)
                return name
        except (aiohttp.ClientError, asyncio.TimeoutError) as e:
            logger.debug("サモナーネーム取得リトライ: %s", e)
        except Exception as e:
            logger.error("予期せぬエラー: %s", e)
        await asyncio.sleep(2)

# ---- ゲーム終了検知 ----
async def wait_for_game_end(session: aiohttp.ClientSession):
    logger.info("ゲーム終了検知まで監視中…")
    while True:
        try:
            async with session.get(ALLGAMEDATA_URL, timeout=HTTP_TIMEOUT) as resp:
                data = await resp.json()
            if data.get("gameData", {}).get("gameEnded"):
                logger.info("ゲーム終了検知！待ち受けに戻ります。")
                break
        except (aiohttp.ClientError, asyncio.TimeoutError) as e:
            logger.warning("ゲーム終了監視中のAPIエラー: %s", e)
        except Exception as e:
            logger.error("予期せぬエラー: %s", e)
        await asyncio.sleep(2)

# ---- メインループ ----
async def main():
    connector = aiohttp.TCPConnector(ssl=False)
    async with aiohttp.ClientSession(connector=connector) as session:
        custom_manager = CustomEventManager()
        history = []

        # イベントハンドラ
        async def on_teamfight(event):
            logger.info("🔥 集団戦検出！%s", event)
            await trigger_obs_replay()
        async def on_my_multikill(event):
            logger.info("🏆 自分のマルチキル！%s", event)
            await trigger_obs_replay()
        async def on_my_death(event):
            logger.info("💀 自分がデス…%s", event)
            await trigger_obs_replay()

        custom_manager.register("TeamFight", on_teamfight)
        custom_manager.register("MyMultikill", on_my_multikill)
        custom_manager.register("MyDeath", on_my_death)

        while True:
            my_name = await get_summoner_name(session)
            logger.info("自分のサモナーネーム: %s", my_name)

            # ゲーム進行中ループ
            while True:
                try:
                    async with session.get(ALLGAMEDATA_URL, timeout=HTTP_TIMEOUT) as resp:
                        data = await resp.json()
                    history.append(data)
                    if len(history) > 10:
                        history.pop(0)
                    await custom_manager.check_all(history, my_name)
                    if data.get("gameData", {}).get("gameEnded"):
                        break
                except (aiohttp.ClientError, asyncio.TimeoutError) as e:
                    logger.warning("API取得失敗: %s", e)
                except Exception as e:
                    logger.error("予期せぬエラー: %s", e)
                await asyncio.sleep(1)
            await wait_for_game_end(session)

if __name__ == "__main__":
    asyncio.run(main())

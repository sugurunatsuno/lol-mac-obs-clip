import asyncio
import websockets
import json
from utils.logger import logger
from utils.option import Option, Some, None_
from utils.result import Result, Ok, Err

OBS_WS_URL = "ws://localhost:4455"

obs_ws: Option[websockets.WebSocketClientProtocol] = None_()


async def get_obs_connection() -> Result[websockets.WebSocketClientProtocol, str]:
    global obs_ws

    if obs_ws.is_none() or not obs_ws.unwrap().open:
        try:
            ws = await websockets.connect(OBS_WS_URL)
            logger.info("OBSとの接続が確立されたよ〜！")

            identify_payload = json.dumps({
                "op": 1,
                "d": {
                    "rpcVersion": 1,
                    "authentication": ""
                }
            })

            await ws.send(identify_payload)
            response = await ws.recv()
            logger.info(f"OBSからのIdentifyレスポンス: {response}")

            obs_ws = Some(ws)
        except Exception as e:
            logger.error(f"❌ OBS接続またはIdentifyエラー: {e}")
            obs_ws = None_()
            return Err(str(e))

    return Ok(obs_ws.unwrap())


async def trigger_replay_buffer() -> Result[None, str]:
    connection_result = await get_obs_connection()

    if connection_result.is_ok():
        try:
            ws = connection_result.unwrap()
            request_payload = json.dumps({
                "op": 6,
                "d": {
                    "requestType": "SaveReplayBuffer",
                    "requestId": "saveReplay"
                }
            })

            await ws.send(request_payload)
            response = await ws.recv()
            logger.info(f"🎬 リプレイ保存のレスポンス: {response}")
            return Ok(None)
        except Exception as e:
            logger.error(f"❌ リプレイ保存リクエストエラー: {e}")
            return Err(str(e))
    else:
        return Err(connection_result.unwrap_err())

# utils/event_types.py（例）

from enum import Enum

# 🌟 APIで取得できるイベント（LoL公式）
class EventType(str, Enum):
    CHAMPION_KILL = "ChampionKill"
    MULTIKILL = "Multikill"
    ACE = "Ace"
    BUILDING_KILL = "BuildingKill"
    DRAGON_KILL = "DragonKill"
    HERALD_KILL = "HeraldKill"
    BARON_KILL = "BaronKill"
    GAME_END = "GameEnd"
    PLAYER_DEATH = "ChampionDeath"
    DRAGON_STEAL = "DragonSteal"
    GRABS_STEAL = "GrabsSteal"
    HERALD_STEAL = "HeraldSteal"
    BARON_STEAL = "BaronSteal"

# 🌟 ライブラリ側で提供するカスタムイベント
class CustomEventType(str, Enum):
    TEAM_FIGHT = "team_fight"
    GOLD_SPIKE = "gold_spike"
    SOLO_BARAM = "solo_baron"
    COMEBACK = "comeback_detected"

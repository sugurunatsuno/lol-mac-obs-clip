#![allow(non_snake_case, non_camel_case_types, dead_code, unused_imports)]

// League of Legends data structures
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct AllGameData {
    pub activePlayer: ActivePlayer,
    pub allPlayers: Vec<Player>,
    pub events: EventData,
    pub gameData: GameData,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct ActivePlayer {
    pub abilities: Abilities,
    pub championStats: ChampionStats,
    pub currentGold: f64,
    pub fullRunes: FullRunes,
    pub level: u32,
    pub riotId: String,
    pub riotIdGameName: String,
    pub riotIdTagLine: String,
    pub summonerName: String,
    pub teamRelativeColors: bool,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct Abilities {
    #[serde(rename = "Q")]
    pub q: Option<Ability>,
    #[serde(rename = "W")]
    pub w: Option<Ability>,
    #[serde(rename = "E")]
    pub e: Option<Ability>,
    #[serde(rename = "R")]
    pub r: Option<Ability>,
    #[serde(rename = "Passive")]
    pub passive: Option<Ability>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct Ability {
    #[serde(default)]
    pub abilityLevel: Option<u32>,
    pub displayName: String,
    pub id: String,
    pub rawDescription: String,
    pub rawDisplayName: String,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct ChampionStats {
    pub abilityHaste: f64,
    pub abilityPower: f64,
    pub armor: f64,
    pub armorPenetrationFlat: f64,
    pub armorPenetrationPercent: f64,
    pub attackDamage: f64,
    pub attackRange: f64,
    pub attackSpeed: f64,
    pub bonusArmorPenetrationPercent: f64,
    pub bonusMagicPenetrationPercent: f64,
    pub critChance: f64,
    pub critDamage: f64,
    pub currentHealth: f64,
    pub healShieldPower: f64,
    pub healthRegenRate: f64,
    pub lifeSteal: f64,
    pub magicLethality: f64,
    pub magicPenetrationFlat: f64,
    pub magicPenetrationPercent: f64,
    pub magicResist: f64,
    pub maxHealth: f64,
    pub moveSpeed: f64,
    pub omnivamp: f64,
    pub physicalLethality: f64,
    pub physicalVamp: f64,
    pub resourceMax: f64,
    pub resourceRegenRate: f64,
    pub resourceType: String,
    pub resourceValue: f64,
    pub spellVamp: f64,
    pub tenacity: f64,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct FullRunes {
    pub generalRunes: Vec<Rune>,
    pub keystone: Rune,
    pub primaryRuneTree: RuneTree,
    pub secondaryRuneTree: RuneTree,
    pub statRunes: Vec<StatRune>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct Rune {
    pub displayName: String,
    pub id: u32,
    pub rawDescription: String,
    pub rawDisplayName: String,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct RuneTree {
    pub displayName: String,
    pub id: u32,
    pub rawDescription: String,
    pub rawDisplayName: String,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct StatRune {
    pub id: u32,
    pub rawDescription: String,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct Player {
    pub championName: String,
    pub isBot: bool,
    pub isDead: bool,
    pub items: Vec<Item>,
    pub level: u32,
    pub position: String,
    pub rawChampionName: String,
    pub rawSkinName: String,
    pub respawnTimer: f64,
    pub riotId: String,
    pub riotIdGameName: String,
    pub riotIdTagLine: String,
    pub runes: PlayerRunes,
    pub scores: Scores,
    pub skinID: i32,
    pub skinName: String,
    pub summonerName: String,
    pub summonerSpells: SummonerSpells,
    pub team: String,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct Item {
    pub canUse: bool,
    pub consumable: bool,
    pub count: u32,
    pub displayName: String,
    pub itemID: i32,
    pub price: u32,
    pub rawDescription: String,
    pub rawDisplayName: String,
    pub slot: u32,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct PlayerRunes {
    pub keystone: Rune,
    pub primaryRuneTree: RuneTree,
    pub secondaryRuneTree: RuneTree,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct Scores {
    pub assists: u32,
    pub creepScore: u32,
    pub deaths: u32,
    pub kills: u32,
    pub wardScore: f64,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct SummonerSpells {
    pub summonerSpellOne: SummonerSpell,
    pub summonerSpellTwo: SummonerSpell,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct SummonerSpell {
    pub displayName: String,
    pub rawDescription: String,
    pub rawDisplayName: String,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct EventData {
    #[serde(rename = "Events")]
    pub events: Vec<LolEvent>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct LolEvent {
    pub EventID: i64,
    pub EventName: String,
    pub EventTime: f64,

    pub Assisters: Option<Vec<String>>,
    pub KillerName: Option<String>,
    pub VictimName: Option<String>,
    pub KillStreak: Option<u32>,
    pub Recipient: Option<String>,
    pub Result: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct GameData {
    pub gameMode: String,
    pub gameTime: f64,
    pub mapName: String,
    pub mapNumber: i32,
    pub mapTerrain: String,
}

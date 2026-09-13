use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};
use sqlx::{MySqlPool, QueryBuilder};

use crate::config::DatabaseConfig;

pub async fn connect(cfg: &DatabaseConfig) -> MySqlPool {
    let options = MySqlConnectOptions::new()
        .host(&cfg.host)
        .port(cfg.port)
        .username(&cfg.username)
        .password(&cfg.password)
        .database(&cfg.database);

    MySqlPoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .expect("failed to connect to the database -- check config.toml and that the server is reachable")
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CharacterRow {
    pub guid: u32,
    pub name: String,
    pub race: u8,
    pub class: u8,
    pub level: u8,
    pub map: u16,
    pub zone: u16,
    pub online: u8,
    pub health: u32,
    pub position_x: f32,
    pub position_y: f32,
}

pub async fn fetch_online_characters(pool: &MySqlPool) -> sqlx::Result<Vec<CharacterRow>> {
    sqlx::query_as::<_, CharacterRow>(
        "SELECT guid, CONVERT(name USING utf8mb4) AS name, race, class, level, map, zone, online, health, \
                position_x, position_y \
         FROM acore_characters.characters WHERE online = 1",
    )
    .fetch_all(pool)
    .await
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AchievementRow {
    pub guid: u32,
    pub achievement: u32,
    // `character_achievement.date` is INT UNSIGNED -- decoding it as i64
    // panics the whole poll tick the moment a real row shows up (confirmed
    // live: this silently broke every poll, roster included, from the
    // first achievement earned after each app restart).
    pub date: u32,
}

pub async fn fetch_new_achievements(pool: &MySqlPool, since_epoch: i64) -> sqlx::Result<Vec<AchievementRow>> {
    sqlx::query_as::<_, AchievementRow>(
        "SELECT guid, achievement, date FROM acore_characters.character_achievement \
         WHERE date > ? ORDER BY date ASC",
    )
    .bind(since_epoch)
    .fetch_all(pool)
    .await
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GuildEventRow {
    pub guildid: u32,
    #[sqlx(rename = "LogGuid")]
    pub log_guid: u32,
    #[sqlx(rename = "EventType")]
    pub event_type: u8,
    #[sqlx(rename = "PlayerGuid1")]
    pub player_guid1: u32,
    #[sqlx(rename = "PlayerGuid2")]
    pub player_guid2: u32,
    // `guild_eventlog.TimeStamp` is also INT UNSIGNED -- same decode-panic
    // hazard as AchievementRow.date above.
    #[sqlx(rename = "TimeStamp")]
    pub timestamp: u32,
}

pub async fn fetch_new_guild_events(pool: &MySqlPool, since_epoch: i64) -> sqlx::Result<Vec<GuildEventRow>> {
    sqlx::query_as::<_, GuildEventRow>(
        "SELECT guildid, LogGuid, EventType, PlayerGuid1, PlayerGuid2, TimeStamp \
         FROM acore_characters.guild_eventlog WHERE TimeStamp > ? ORDER BY TimeStamp ASC",
    )
    .bind(since_epoch)
    .fetch_all(pool)
    .await
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GroupMemberRow {
    pub guid: u32,
    #[sqlx(rename = "memberGuid")]
    pub member_guid: u32,
}

pub async fn fetch_group_members(pool: &MySqlPool) -> sqlx::Result<Vec<GroupMemberRow>> {
    sqlx::query_as::<_, GroupMemberRow>("SELECT guid, memberGuid FROM acore_characters.group_member")
        .fetch_all(pool)
        .await
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ChatRow {
    pub id: u64,
    // `mod_ollama_chat_history.bot_guid`/`player_guid` are both BIGINT
    // UNSIGNED (not the usual INT UNSIGNED character guids use elsewhere)
    // -- decoding as u32 has the same panic hazard as the columns above.
    pub bot_guid: u64,
    pub player_guid: u64,
    pub player_message: String,
    pub bot_reply: String,
}

pub async fn fetch_new_chat(pool: &MySqlPool, since_id: u64) -> sqlx::Result<Vec<ChatRow>> {
    sqlx::query_as::<_, ChatRow>(
        "SELECT id, bot_guid, player_guid, player_message, bot_reply \
         FROM acore_characters.mod_ollama_chat_history WHERE id > ? ORDER BY id ASC",
    )
    .bind(since_id)
    .fetch_all(pool)
    .await
}

/// A completed dungeon/raid encounter, as logged by AzerothCore's
/// `log_encounter` table. `credit_type` follows AzerothCore's
/// `EncounterCreditType`: 0 = kill credit (`credit_entry` is a
/// `creature_template.entry`, resolved to `boss_name`), 1 = cast-spell
/// credit (`credit_entry` is a spell id instead -- `boss_name` will be
/// `None` since it won't match any creature).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BossKillRow {
    pub time_epoch: i64,
    pub map: u16,
    pub difficulty: u8,
    pub credit_type: u8,
    pub credit_entry: u32,
    pub boss_name: Option<String>,
}

pub async fn fetch_new_boss_kills(pool: &MySqlPool, since_epoch: i64) -> sqlx::Result<Vec<BossKillRow>> {
    sqlx::query_as::<_, BossKillRow>(
        "SELECT UNIX_TIMESTAMP(le.time) AS time_epoch, le.map, le.difficulty, \
                le.creditType AS credit_type, le.creditEntry AS credit_entry, \
                CONVERT(ct.name USING utf8mb4) AS boss_name \
         FROM acore_characters.log_encounter le \
         LEFT JOIN acore_world.creature_template ct ON le.creditEntry = ct.entry \
         WHERE le.time > FROM_UNIXTIME(?) ORDER BY le.time ASC",
    )
    .bind(since_epoch)
    .fetch_all(pool)
    .await
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GuildNameRow {
    pub guildid: u32,
    pub name: String,
}

pub async fn fetch_guild_names(pool: &MySqlPool) -> sqlx::Result<Vec<GuildNameRow>> {
    sqlx::query_as::<_, GuildNameRow>(
        "SELECT guildid, CONVERT(name USING utf8mb4) AS name FROM acore_characters.guild",
    )
    .fetch_all(pool)
    .await
}

/// `character_queststatus_rewarded` has no timestamp column, so unlike
/// achievements/guild events this can't be polled with `WHERE date > ?`.
/// Instead the poller tracks each online bot's known completed-quest IDs in
/// memory and diffs against that; this just returns every rewarded quest
/// currently on record for the given (online) guids, joined against
/// `quest_template` for a real quest name.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct QuestCompletionRow {
    pub guid: u32,
    pub quest: u32,
    pub quest_name: Option<String>,
}

pub async fn fetch_quest_completions(pool: &MySqlPool, guids: &[u32]) -> sqlx::Result<Vec<QuestCompletionRow>> {
    if guids.is_empty() {
        return Ok(Vec::new());
    }

    let mut qb = QueryBuilder::new(
        "SELECT cqr.guid, cqr.quest, CONVERT(qt.LogTitle USING utf8mb4) AS quest_name \
         FROM acore_characters.character_queststatus_rewarded cqr \
         LEFT JOIN acore_world.quest_template qt ON cqr.quest = qt.ID \
         WHERE cqr.guid IN (",
    );
    let mut separated = qb.separated(", ");
    for guid in guids {
        separated.push_bind(guid);
    }
    qb.push(")");

    qb.build_query_as::<QuestCompletionRow>().fetch_all(pool).await
}

/// `item_instance`/`character_inventory` have no acquisition timestamp
/// either (they only reflect current possession, not history) -- same
/// known-set-diffing approach as quests. `item_guid` is `item_instance.guid`
/// (a stable per-item id, distinct from `item_entry` which just identifies
/// the item *type* and repeats across every copy of that item), so it's
/// what the poller tracks to detect "this specific item is new to this bot".
///
/// `it.stackable <= 1` excludes anything that can stack (ammo, consumables,
/// trade goods): confirmed live that a stackable item's `item_instance.guid`
/// gets recreated whenever the game reshuffles/splits that stack, which the
/// guid-diffing above misreads as "found a new item" every time -- observed
/// as the same bot "finding" the same stack of arrows six times in a row.
/// Real gear (weapons/armor) is always stackable=1, so this loses nothing
/// we actually want to notify on.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LootRow {
    pub guid: u32,
    pub item_guid: u32,
    pub item_name: Option<String>,
    pub quality: u8,
}

pub async fn fetch_notable_loot(pool: &MySqlPool, guids: &[u32]) -> sqlx::Result<Vec<LootRow>> {
    if guids.is_empty() {
        return Ok(Vec::new());
    }

    let mut qb = QueryBuilder::new(
        "SELECT ci.guid, ii.guid AS item_guid, CONVERT(it.name USING utf8mb4) AS item_name, it.Quality AS quality \
         FROM acore_characters.character_inventory ci \
         JOIN acore_characters.item_instance ii ON ci.item = ii.guid \
         JOIN acore_world.item_template it ON ii.itemEntry = it.entry \
         WHERE it.Quality >= 2 AND it.stackable <= 1 AND ci.guid IN (",
    );
    let mut separated = qb.separated(", ");
    for guid in guids {
        separated.push_bind(guid);
    }
    qb.push(")");

    qb.build_query_as::<LootRow>().fetch_all(pool).await
}

/// WotLK 3.3.5a skill line IDs for the three gathering professions --
/// confirmed live against real bot data (values in the expected 1-400
/// skill-cap range, hundreds of online bots holding each one). No real
/// display-name source exists on this install (`skillline_dbc` has 0 rows,
/// same situation as map_dbc/dungeonencounter_dbc before those were
/// extracted from the client), but these three names are common knowledge
/// and unambiguous, unlike the numerous achievement/zone names that
/// genuinely needed extraction -- see `poller::gathering_skill_name`.
pub const SKILL_HERBALISM: u16 = 182;
pub const SKILL_MINING: u16 = 186;
pub const SKILL_SKINNING: u16 = 393;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SkillRow {
    pub guid: u32,
    pub skill: u16,
    pub value: u16,
}

pub async fn fetch_gathering_skills(pool: &MySqlPool, guids: &[u32]) -> sqlx::Result<Vec<SkillRow>> {
    if guids.is_empty() {
        return Ok(Vec::new());
    }

    let mut qb = QueryBuilder::new(
        "SELECT guid, skill, value FROM acore_characters.character_skills \
         WHERE skill IN (182, 186, 393) AND guid IN (",
    );
    let mut separated = qb.separated(", ");
    for guid in guids {
        separated.push_bind(guid);
    }
    qb.push(")");

    qb.build_query_as::<SkillRow>().fetch_all(pool).await
}

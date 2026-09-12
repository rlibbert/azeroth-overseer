use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};
use sqlx::MySqlPool;

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
}

pub async fn fetch_online_characters(pool: &MySqlPool) -> sqlx::Result<Vec<CharacterRow>> {
    sqlx::query_as::<_, CharacterRow>(
        "SELECT guid, CONVERT(name USING utf8mb4) AS name, race, class, level, map, zone, online \
         FROM acore_characters.characters WHERE online = 1",
    )
    .fetch_all(pool)
    .await
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AchievementRow {
    pub guid: u32,
    pub achievement: u32,
    pub date: i64,
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
    #[sqlx(rename = "TimeStamp")]
    pub timestamp: i64,
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
    pub bot_guid: u32,
    pub player_guid: u32,
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

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::MySqlPool;
use tokio::time::{interval, Duration};

use crate::character_info::{class_name, race_name};
use crate::db;
use crate::events::Event;
use crate::names::NameTables;

#[derive(Debug, Clone)]
struct CharacterSnapshot {
    name: String,
    race: u8,
    class: u8,
    level: u8,
    zone: u16,
}

pub struct PollerState {
    characters: HashMap<u32, CharacterSnapshot>,
    last_achievement_epoch: i64,
    last_guild_event_epoch: i64,
    last_chat_id: u64,
    groups: HashMap<u32, Vec<u32>>,
    guild_names: HashMap<u32, String>,
    first_tick: bool,
}

impl PollerState {
    pub fn new() -> Self {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
        Self {
            characters: HashMap::new(),
            // Start from "now" so the very first tick doesn't dump the
            // entire 10,000+ row achievement/guild-event history as events.
            last_achievement_epoch: now,
            last_guild_event_epoch: now,
            last_chat_id: 0,
            groups: HashMap::new(),
            guild_names: HashMap::new(),
            first_tick: true,
        }
    }
}

/// Runs the poll loop forever, calling `on_events` with each tick's
/// synthesized events. Kept generic over the callback (rather than directly
/// wired to a WebSocket broadcaster) so Phase 2 can drive it with a simple
/// stdout printer before the WebSocket layer exists.
pub async fn run<F>(pool: MySqlPool, names: NameTables, poll_interval_secs: u64, mut on_events: F)
where
    F: FnMut(&[Event]),
{
    let mut state = PollerState::new();
    let mut ticker = interval(Duration::from_secs(poll_interval_secs));

    loop {
        ticker.tick().await;
        match poll_once(&pool, &names, &mut state).await {
            Ok(events) => {
                if !events.is_empty() {
                    on_events(&events);
                }
            }
            Err(e) => eprintln!("poll error (will retry next tick): {e}"),
        }
    }
}

async fn poll_once(pool: &MySqlPool, names: &NameTables, state: &mut PollerState) -> sqlx::Result<Vec<Event>> {
    let mut events = Vec::new();

    // Refresh guild names occasionally -- cheap table, just reload every tick.
    let guild_rows = db::fetch_guild_names(pool).await?;
    state.guild_names = guild_rows.into_iter().map(|g| (g.guildid, g.name)).collect();

    // --- Characters: online/offline, level, zone ---
    let rows = db::fetch_online_characters(pool).await?;
    let mut new_snapshot: HashMap<u32, CharacterSnapshot> = HashMap::new();

    for row in &rows {
        let snap = CharacterSnapshot {
            name: row.name.clone(),
            race: row.race,
            class: row.class,
            level: row.level,
            zone: row.zone,
        };

        match state.characters.get(&row.guid) {
            None => {
                if !state.first_tick {
                    events.push(Event::WentOnline {
                        guid: row.guid,
                        name: row.name.clone(),
                        race: race_name(row.race).to_string(),
                        class: class_name(row.class).to_string(),
                        level: row.level,
                    });
                }
            }
            Some(old) => {
                if row.level > old.level {
                    events.push(Event::LevelUp {
                        guid: row.guid,
                        name: row.name.clone(),
                        class: class_name(row.class).to_string(),
                        old_level: old.level,
                        new_level: row.level,
                    });
                }
                if row.zone != old.zone && row.zone != 0 {
                    events.push(Event::ZoneChanged {
                        guid: row.guid,
                        name: row.name.clone(),
                        zone_name: names.zone_name(row.zone as u32),
                    });
                }
            }
        }

        new_snapshot.insert(row.guid, snap);
    }

    if !state.first_tick {
        for (guid, old) in &state.characters {
            if !new_snapshot.contains_key(guid) {
                events.push(Event::WentOffline {
                    guid: *guid,
                    name: old.name.clone(),
                });
            }
        }
    }
    state.characters = new_snapshot;

    // --- Achievements ---
    let achievements = db::fetch_new_achievements(pool, state.last_achievement_epoch).await?;
    for row in &achievements {
        let name = state
            .characters
            .get(&row.guid)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| format!("Character #{}", row.guid));
        events.push(Event::AchievementEarned {
            guid: row.guid,
            name,
            achievement_name: names.achievement_name(row.achievement),
        });
        state.last_achievement_epoch = state.last_achievement_epoch.max(row.date);
    }

    // --- Guild events ---
    let guild_events = db::fetch_new_guild_events(pool, state.last_guild_event_epoch).await?;
    for row in &guild_events {
        let guild_name = state
            .guild_names
            .get(&row.guildid)
            .cloned()
            .unwrap_or_else(|| format!("Guild #{}", row.guildid));
        let actor = state
            .characters
            .get(&row.player_guid1)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| format!("Character #{}", row.player_guid1));
        let description = describe_guild_event(row.event_type, &actor, row.player_guid2, state);
        events.push(Event::GuildActivity { guild_name, description });
        state.last_guild_event_epoch = state.last_guild_event_epoch.max(row.timestamp);
    }

    // --- Groups ---
    let member_rows = db::fetch_group_members(pool).await?;
    let mut new_groups: HashMap<u32, Vec<u32>> = HashMap::new();
    for row in member_rows {
        new_groups.entry(row.guid).or_default().push(row.member_guid);
    }
    for members in new_groups.values_mut() {
        members.sort_unstable();
    }

    if !state.first_tick {
        for (group_id, members) in &new_groups {
            if !state.groups.contains_key(group_id) {
                events.push(Event::GroupFormed {
                    member_names: resolve_names(members, state),
                });
            }
        }
        for (group_id, members) in &state.groups {
            if !new_groups.contains_key(group_id) {
                events.push(Event::GroupDisbanded {
                    member_names: resolve_names(members, state),
                });
            }
        }
    }
    state.groups = new_groups;

    // --- Chat (bonus channel, usually empty) ---
    let chat_rows = db::fetch_new_chat(pool, state.last_chat_id).await?;
    for row in &chat_rows {
        let bot_name = state
            .characters
            .get(&row.bot_guid)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| format!("Character #{}", row.bot_guid));
        events.push(Event::ChatExchange {
            bot_name,
            player_message: row.player_message.clone(),
            bot_reply: row.bot_reply.clone(),
        });
        state.last_chat_id = state.last_chat_id.max(row.id);
    }

    state.first_tick = false;
    Ok(events)
}

fn resolve_names(guids: &[u32], state: &PollerState) -> Vec<String> {
    guids
        .iter()
        .map(|g| {
            state
                .characters
                .get(g)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| format!("Character #{g}"))
        })
        .collect()
}

/// AzerothCore's guild_eventlog EventType codes. Exact mapping to be
/// cross-checked against real observed events during Phase 2's live
/// terminal-log exit criteria -- unknown codes fall back to a generic
/// description rather than guessing further.
fn describe_guild_event(event_type: u8, actor: &str, _target_guid: u32, _state: &PollerState) -> String {
    match event_type {
        0 => format!("{actor} was invited to the guild"),
        1 => format!("{actor} joined the guild"),
        2 => format!("{actor} left the guild"),
        3 => format!("{actor} was kicked from the guild"),
        4 => format!("{actor} was promoted"),
        5 => format!("{actor} was demoted"),
        6 => "Guild leadership changed".to_string(),
        7 => "A guild rank was created".to_string(),
        8 => "A guild rank was removed".to_string(),
        other => format!("{actor} triggered a guild event (type {other})"),
    }
}

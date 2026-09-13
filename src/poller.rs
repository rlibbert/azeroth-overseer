use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::MySqlPool;
use tokio::time::{interval, Duration};

use crate::character_info::{class_name, race_name};
use crate::db;
use crate::events::{Event, RosterEntry};
use crate::names::NameTables;

#[derive(Debug, Clone)]
struct CharacterSnapshot {
    name: String,
    race: u8,
    class: u8,
    level: u8,
    zone: u16,
    health: u32,
    map: u16,
    position_x: f32,
    position_y: f32,
}

// Verified directly from AzerothCore's own source (not guessed): the engine
// divides every continent into a 64x64 grid of 533.3333-yard cells --
// src/common/Collision/Maps/MapDefines.h's SIZE_OF_GRIDS/MAX_NUMBER_OF_GRIDS,
// the same constants GridDefines.h's own world-to-grid conversion uses. This
// is continent-independent (Eastern Kingdoms, Kalimdor, Outland, and
// Northrend all share the same 64x64/533.3333 convention), so no per-map
// data is needed.
const GRID_SIZE_YARDS: f32 = 533.3333;
const GRID_COUNT: f32 = 64.0;
const CENTER_GRID: f32 = GRID_COUNT / 2.0;

/// WoW's world axes: +X is north, +Y is west (confirmed by AzerothCore's own
/// `Zone2MapCoordinates`, which swaps x/y when converting world coordinates
/// to the client's map-image axes for exactly this reason). Screen space
/// wants x=west-to-east, y=north-to-south, so `position_y` drives the
/// returned x and `position_x` drives the returned y. Returns fractions in
/// roughly 0..1, clamped, since a character can technically sit slightly
/// outside the nominal continent-sized square near map edges.
fn world_to_screen(position_x: f32, position_y: f32) -> (f32, f32) {
    let grid_x = CENTER_GRID - position_y / GRID_SIZE_YARDS;
    let grid_y = CENTER_GRID - position_x / GRID_SIZE_YARDS;
    ((grid_x / GRID_COUNT).clamp(0.0, 1.0), (grid_y / GRID_COUNT).clamp(0.0, 1.0))
}

pub struct PollerState {
    characters: HashMap<u32, CharacterSnapshot>,
    last_achievement_epoch: i64,
    last_guild_event_epoch: i64,
    last_boss_kill_epoch: i64,
    last_chat_id: u64,
    groups: HashMap<u32, Vec<u32>>,
    guild_names: HashMap<u32, String>,
    // `character_queststatus_rewarded` has no timestamp column, so instead
    // of `WHERE date > last_epoch` we track each bot's known completed-quest
    // IDs here and diff against that. `seeded_quest_guids` marks which bots
    // have had their existing history silently absorbed already, so a bot's
    // first-ever appearance doesn't dump 20+ quests as "just completed".
    known_quests: HashMap<u32, HashSet<u32>>,
    seeded_quest_guids: HashSet<u32>,
    // Same pattern for notable (quality >= uncommon) items: `item_instance`
    // has no acquisition timestamp, so we track each bot's known item_guids
    // (not item *types* -- a bot can hold two copies of the same item) and
    // diff against that, seeding silently on first sight.
    known_items: HashMap<u32, HashSet<u32>>,
    seeded_item_guids: HashSet<u32>,
    // Gathering skill values (Mining/Herbalism/Skinning) -- unlike the
    // quest/item sets above this is a single increasing number per
    // (guid, skill), so it's tracked the same way as CharacterSnapshot's
    // `level`: first sighting seeds silently, later sightings diff.
    skill_values: HashMap<(u32, u16), u16>,
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
            last_boss_kill_epoch: now,
            last_chat_id: 0,
            groups: HashMap::new(),
            guild_names: HashMap::new(),
            known_quests: HashMap::new(),
            seeded_quest_guids: HashSet::new(),
            known_items: HashMap::new(),
            seeded_item_guids: HashSet::new(),
            skill_values: HashMap::new(),
            first_tick: true,
        }
    }
}

/// Runs the poll loop forever. `on_events` is called with each tick's
/// synthesized delta events (only when non-empty, for the ticker);
/// `on_roster` is called every tick with the full current online roster
/// (even when empty), since the frontend needs the whole agent field and
/// aggregate stats, not just deltas. Kept generic over both callbacks
/// (rather than directly wired to a WebSocket broadcaster) so Phase 2 could
/// drive this with a simple stdout printer before the WebSocket layer existed.
pub async fn run<F, G>(
    pool: MySqlPool,
    names: NameTables,
    poll_interval_secs: u64,
    mut on_events: F,
    mut on_roster: G,
) where
    F: FnMut(&[Event]),
    G: FnMut(&[RosterEntry]),
{
    let mut state = PollerState::new();
    let mut ticker = interval(Duration::from_secs(poll_interval_secs));

    loop {
        ticker.tick().await;
        match poll_once(&pool, &names, &mut state).await {
            Ok((events, roster)) => {
                if !events.is_empty() {
                    on_events(&events);
                }
                on_roster(&roster);
            }
            Err(e) => eprintln!("poll error (will retry next tick): {e}"),
        }
    }
}

async fn poll_once(
    pool: &MySqlPool,
    names: &NameTables,
    state: &mut PollerState,
) -> sqlx::Result<(Vec<Event>, Vec<RosterEntry>)> {
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
            health: row.health,
            map: row.map,
            position_x: row.position_x,
            position_y: row.position_y,
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
                // `health` hitting 0 is the only reliable "just died" signal
                // on this schema -- `death_expire_time` stays nonzero long
                // after a character revives, so it can't be used as a
                // point-in-time trigger (confirmed live: ~85% of currently
                // online, clearly-alive characters had it set).
                if row.health == 0 && old.health > 0 {
                    events.push(Event::Death {
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
        state.last_achievement_epoch = state.last_achievement_epoch.max(row.date as i64);
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
        state.last_guild_event_epoch = state.last_guild_event_epoch.max(row.timestamp as i64);
    }

    // --- Boss kills (bonus channel: `log_encounter` only gets rows when a
    // group actually clears a dungeon/raid encounter, which Playerbots
    // groups do rarely compared to solo questing -- expect this to be sparse
    // or silent for long stretches, same as the chat/guild channels). ---
    let boss_kills = db::fetch_new_boss_kills(pool, state.last_boss_kill_epoch).await?;
    for row in &boss_kills {
        let boss_name = describe_boss(row.credit_type, row.credit_entry, row.boss_name.as_deref());
        events.push(Event::BossKill { boss_name, map_name: names.map_name(row.map as u32) });
        state.last_boss_kill_epoch = state.last_boss_kill_epoch.max(row.time_epoch);
    }

    // --- Quest completions (no timestamp column -- diff against each bot's
    // known completed-quest set instead; see PollerState's known_quests). ---
    let online_guids: Vec<u32> = rows.iter().map(|r| r.guid).collect();
    let quest_rows = db::fetch_quest_completions(pool, &online_guids).await?;
    let mut quests_by_guid: HashMap<u32, Vec<&db::QuestCompletionRow>> = HashMap::new();
    for row in &quest_rows {
        quests_by_guid.entry(row.guid).or_default().push(row);
    }
    for (guid, guid_quest_rows) in &quests_by_guid {
        let known = state.known_quests.entry(*guid).or_default();
        // `insert` on a HashSet not seen before returns true for this bot's
        // very first tick, so `first_time` tells us whether to seed silently.
        let first_time = state.seeded_quest_guids.insert(*guid);
        for row in guid_quest_rows {
            if known.insert(row.quest) && !first_time {
                let name = state
                    .characters
                    .get(guid)
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| format!("Character #{guid}"));
                let quest_name = row
                    .quest_name
                    .clone()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| format!("Quest #{}", row.quest));
                events.push(Event::QuestCompleted { guid: *guid, name, quest_name });
            }
        }
    }

    // --- Notable loot (quality >= uncommon/"green") -- same known-set-diffing
    // approach as quest completions, see PollerState's known_items. ---
    let loot_rows = db::fetch_notable_loot(pool, &online_guids).await?;
    let mut loot_by_guid: HashMap<u32, Vec<&db::LootRow>> = HashMap::new();
    for row in &loot_rows {
        loot_by_guid.entry(row.guid).or_default().push(row);
    }
    for (guid, guid_loot_rows) in &loot_by_guid {
        let known = state.known_items.entry(*guid).or_default();
        let first_time = state.seeded_item_guids.insert(*guid);
        for row in guid_loot_rows {
            if known.insert(row.item_guid) && !first_time {
                let name = state
                    .characters
                    .get(guid)
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| format!("Character #{guid}"));
                let item_name = row
                    .item_name
                    .clone()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| format!("Item #{}", row.item_guid));
                events.push(Event::ItemFound { guid: *guid, name, item_name, quality: row.quality });
            }
        }
    }

    // --- Gathering skill-ups (Mining/Herbalism/Skinning). Note this signals
    // "successfully gathered at some point since last tick", not a
    // per-node/per-gather log -- AzerothCore doesn't keep one, and skill-ups
    // are themselves probabilistic (same relationship a level-up has to
    // killing mobs, not a 1:1 event log). ---
    let skill_rows = db::fetch_gathering_skills(pool, &online_guids).await?;
    for row in &skill_rows {
        let key = (row.guid, row.skill);
        match state.skill_values.get(&key).copied() {
            None => {
                state.skill_values.insert(key, row.value);
            }
            Some(old_value) if row.value > old_value => {
                let name = state
                    .characters
                    .get(&row.guid)
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| format!("Character #{}", row.guid));
                events.push(Event::SkillUp {
                    guid: row.guid,
                    name,
                    skill_name: gathering_skill_name(row.skill).to_string(),
                    new_value: row.value,
                });
                state.skill_values.insert(key, row.value);
            }
            Some(_) => {}
        }
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
            .get(&(row.bot_guid as u32))
            .map(|c| c.name.clone())
            .unwrap_or_else(|| format!("Character #{}", row.bot_guid));
        events.push(Event::ChatExchange {
            bot_name,
            player_message: row.player_message.clone(),
            bot_reply: row.bot_reply.clone(),
        });
        state.last_chat_id = state.last_chat_id.max(row.id);
    }

    // --- Build the full roster for the frontend's agent field + stats ---
    let mut member_to_group: HashMap<u32, u32> = HashMap::new();
    for (group_id, members) in &state.groups {
        for m in members {
            member_to_group.insert(*m, *group_id);
        }
    }
    let roster: Vec<RosterEntry> = state
        .characters
        .iter()
        .map(|(guid, c)| {
            let (screen_x, screen_y) = world_to_screen(c.position_x, c.position_y);
            RosterEntry {
                guid: *guid,
                name: c.name.clone(),
                race: race_name(c.race).to_string(),
                class: class_name(c.class).to_string(),
                level: c.level,
                zone_name: names.zone_name(c.zone as u32),
                group_id: member_to_group.get(guid).copied(),
                map: c.map,
                map_name: names.map_name(c.map as u32),
                screen_x,
                screen_y,
            }
        })
        .collect();

    state.first_tick = false;
    Ok((events, roster))
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

/// No real display-name source exists for skill IDs on this install (see
/// db::SKILL_HERBALISM's doc comment) -- these three are hardcoded rather
/// than looked up, since they're common knowledge and don't carry the same
/// "don't guess" risk as the achievement/zone names that needed extraction.
fn gathering_skill_name(skill: u16) -> &'static str {
    match skill {
        db::SKILL_HERBALISM => "Herbalism",
        db::SKILL_MINING => "Mining",
        db::SKILL_SKINNING => "Skinning",
        _ => "Gathering",
    }
}

/// `credit_type` 0 = kill credit (`credit_entry` is a creature_template
/// entry, resolved to a real boss name via the join in `db::fetch_new_boss_kills`);
/// 1 = cast-spell credit (`credit_entry` is a spell id instead, so it won't
/// match any creature -- described generically rather than guessed).
fn describe_boss(credit_type: u8, credit_entry: u32, boss_name: Option<&str>) -> String {
    if credit_type == 0 {
        match boss_name {
            Some(name) if !name.is_empty() => name.to_string(),
            _ => format!("Unknown Boss (Entry #{credit_entry})"),
        }
    } else {
        format!("Encounter Objective #{credit_entry}")
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_origin_is_screen_center() {
        // Standing at world (0, 0) -- the center of the 64x64 grid on any
        // continent -- should land dead center of the rendered square.
        let (x, y) = world_to_screen(0.0, 0.0);
        assert!((x - 0.5).abs() < 1e-6, "x={x}");
        assert!((y - 0.5).abs() < 1e-6, "y={y}");
    }

    #[test]
    fn north_is_up_and_west_is_left() {
        // +X is north in WoW's world space; moving north should decrease
        // screen_y (move toward the top). +Y is west; moving west should
        // decrease screen_x (move toward the left).
        let (_, y_north) = world_to_screen(5000.0, 0.0);
        let (_, y_south) = world_to_screen(-5000.0, 0.0);
        assert!(y_north < 0.5 && y_south > 0.5, "y_north={y_north} y_south={y_south}");

        let (x_west, _) = world_to_screen(0.0, 5000.0);
        let (x_east, _) = world_to_screen(0.0, -5000.0);
        assert!(x_west < 0.5 && x_east > 0.5, "x_west={x_west} x_east={x_east}");
    }

    #[test]
    fn clamps_out_of_bounds_coordinates() {
        let (x, y) = world_to_screen(-100_000.0, 100_000.0);
        assert!((0.0..=1.0).contains(&x));
        assert!((0.0..=1.0).contains(&y));
    }
}

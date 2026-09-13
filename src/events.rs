use serde::Serialize;

// NOTE: enum-level `rename_all` only renames the variant tag ("type") --
// it does NOT cascade into each struct variant's own fields (confirmed by
// inspecting the real wire output: "type" came through as "zoneChanged" but
// its field stayed "zone_name", not "zoneName"). Every struct variant below
// therefore needs its own `rename_all` too.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
pub enum Event {
    #[serde(rename_all = "camelCase")]
    WentOnline {
        guid: u32,
        name: String,
        race: String,
        class: String,
        level: u8,
    },
    #[serde(rename_all = "camelCase")]
    WentOffline {
        guid: u32,
        name: String,
    },
    #[serde(rename_all = "camelCase")]
    LevelUp {
        guid: u32,
        name: String,
        class: String,
        old_level: u8,
        new_level: u8,
    },
    #[serde(rename_all = "camelCase")]
    ZoneChanged {
        guid: u32,
        name: String,
        zone_name: String,
    },
    #[serde(rename_all = "camelCase")]
    AchievementEarned {
        guid: u32,
        name: String,
        achievement_name: String,
    },
    #[serde(rename_all = "camelCase")]
    GuildActivity {
        guild_name: String,
        description: String,
    },
    #[serde(rename_all = "camelCase")]
    GroupFormed {
        member_names: Vec<String>,
    },
    #[serde(rename_all = "camelCase")]
    GroupDisbanded {
        member_names: Vec<String>,
    },
    #[serde(rename_all = "camelCase")]
    ChatExchange {
        bot_name: String,
        player_message: String,
        bot_reply: String,
    },
    #[serde(rename_all = "camelCase")]
    Death {
        guid: u32,
        name: String,
        zone_name: String,
    },
    #[serde(rename_all = "camelCase")]
    BossKill {
        boss_name: String,
        map_name: String,
    },
    #[serde(rename_all = "camelCase")]
    QuestCompleted {
        guid: u32,
        name: String,
        quest_name: String,
    },
    #[serde(rename_all = "camelCase")]
    ItemFound {
        guid: u32,
        name: String,
        item_name: String,
        quality: u8,
    },
    #[serde(rename_all = "camelCase")]
    SkillUp {
        guid: u32,
        name: String,
        skill_name: String,
        new_value: u16,
    },
}

/// The full current online roster, broadcast every poll tick alongside
/// (but distinct from) individual `Event`s -- the frontend needs this to
/// draw the whole agent field and compute aggregate stats, not just react
/// to deltas.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RosterEntry {
    pub guid: u32,
    pub name: String,
    pub race: String,
    pub class: String,
    pub level: u8,
    pub zone_name: String,
    pub group_id: Option<u32>,
    pub map: u16,
    pub map_name: String,
    // Pre-computed 0..1 fractional position within this bot's continent,
    // already oriented for screen display (x = west-to-east, y = north-to-
    // south) -- see poller::world_to_screen for the derivation. Shipping
    // the finished fraction rather than raw world coordinates keeps the
    // orientation/axis-swap math in one tested place instead of duplicating
    // it in JS.
    pub screen_x: f32,
    pub screen_y: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RosterMessage {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub entries: Vec<RosterEntry>,
}

impl RosterMessage {
    pub fn new(entries: Vec<RosterEntry>) -> Self {
        Self { kind: "roster", entries }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_variants_serialize_camel_case() {
        let events = vec![
            Event::WentOnline { guid: 1, name: "A".into(), race: "Troll".into(), class: "Rogue".into(), level: 1 },
            Event::WentOffline { guid: 1, name: "A".into() },
            Event::LevelUp { guid: 1, name: "A".into(), class: "Rogue".into(), old_level: 1, new_level: 2 },
            Event::ZoneChanged { guid: 1, name: "A".into(), zone_name: "Durotar".into() },
            Event::AchievementEarned { guid: 1, name: "A".into(), achievement_name: "X".into() },
            Event::GuildActivity { guild_name: "G".into(), description: "d".into() },
            Event::GroupFormed { member_names: vec!["A".into()] },
            Event::GroupDisbanded { member_names: vec!["A".into()] },
            Event::ChatExchange { bot_name: "A".into(), player_message: "hi".into(), bot_reply: "yo".into() },
            Event::Death { guid: 1, name: "A".into(), zone_name: "Durotar".into() },
            Event::BossKill { boss_name: "Onyxia".into(), map_name: "Onyxia's Lair".into() },
            Event::QuestCompleted { guid: 1, name: "A".into(), quest_name: "Jungle Secrets".into() },
            Event::ItemFound { guid: 1, name: "A".into(), item_name: "Zulian Slicer".into(), quality: 3 },
            Event::SkillUp { guid: 1, name: "A".into(), skill_name: "Mining".into(), new_value: 45 },
        ];
        for e in &events {
            let json = serde_json::to_string(e).unwrap();
            println!("{json}");
            assert!(!json.contains('_'), "found snake_case leak in {json}");
        }
    }
}

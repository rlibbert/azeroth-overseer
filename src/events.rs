use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
pub enum Event {
    WentOnline {
        guid: u32,
        name: String,
        race: String,
        class: String,
        level: u8,
    },
    WentOffline {
        guid: u32,
        name: String,
    },
    LevelUp {
        guid: u32,
        name: String,
        class: String,
        old_level: u8,
        new_level: u8,
    },
    ZoneChanged {
        guid: u32,
        name: String,
        zone_name: String,
    },
    AchievementEarned {
        guid: u32,
        name: String,
        achievement_name: String,
    },
    GuildActivity {
        guild_name: String,
        description: String,
    },
    GroupFormed {
        member_names: Vec<String>,
    },
    GroupDisbanded {
        member_names: Vec<String>,
    },
    ChatExchange {
        bot_name: String,
        player_message: String,
        bot_reply: String,
    },
}

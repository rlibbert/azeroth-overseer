mod character_info;
mod config;
mod db;
mod events;
mod names;
mod poller;

use config::Config;
use names::NameTables;

#[tokio::main]
async fn main() {
    let config = Config::load();
    let names = NameTables::load();

    println!(
        "Connecting to {}:{}/{}...",
        config.database.host, config.database.port, config.database.database
    );
    let pool = db::connect(&config.database).await;
    println!("Connected. Polling every {}s. Ctrl+C to stop.\n", config.server.poll_interval_secs);

    poller::run(pool, names, config.server.poll_interval_secs, |events| {
        for event in events {
            println!("{}", format_event_line(event));
        }
    })
    .await;
}

fn format_event_line(event: &events::Event) -> String {
    use events::Event::*;
    match event {
        WentOnline { name, race, class, level, .. } => {
            format!("[ONLINE]  {name} ({level} {race} {class}) logged in")
        }
        WentOffline { name, .. } => format!("[OFFLINE] {name} logged out"),
        LevelUp { name, class, old_level, new_level, .. } => {
            format!("[LEVEL]   {name} the {class} reached level {new_level} (was {old_level})")
        }
        ZoneChanged { name, zone_name, .. } => format!("[ZONE]    {name} entered {zone_name}"),
        AchievementEarned { name, achievement_name, .. } => {
            format!("[ACHIEVE] {name} earned \"{achievement_name}\"")
        }
        GuildActivity { guild_name, description } => format!("[GUILD]   <{guild_name}> {description}"),
        GroupFormed { member_names } => format!("[GROUP]   Formed: {}", member_names.join(", ")),
        GroupDisbanded { member_names } => format!("[GROUP]   Disbanded: {}", member_names.join(", ")),
        ChatExchange { bot_name, player_message, bot_reply } => {
            format!("[CHAT]    {bot_name}: \"{player_message}\" -> \"{bot_reply}\"")
        }
    }
}

# Azeroth Overseer

A live, full-screen presentation console for watching an [AzerothCore](https://www.azerothcore.org/) Playerbots population in real time. It polls the server's database, turns raw state changes into a stream of narrated events, and renders them as a Matrix-style "surveillance feed" — you're the Overseer, watching the bots live in Azeroth.

This is a look-and-feel-first tool built for a live audience, not a GM/admin utility (see [wow-gm-console](https://github.com/rlibbert/wow-gm-console) for that).

![status](https://img.shields.io/badge/status-active%20development-yellow)

## What it shows

- **Agent field** — four radar-style quadrants, one per continent (Eastern Kingdoms, Kalimdor, Outland, Northrend), with every online bot plotted at its *true relative position* on that continent. Dots are colored by class, pulse briefly when that bot triggers an event, and are connected by lines to other bots in the same party.
- **Event feed** — a live ticker narrating what's happening: level-ups, zone changes, deaths, achievements, guild activity, quest completions, notable item finds, gathering skill-ups, party formation, and (when it happens) boss kills.
- **Telemetry** — agents online, average level, and running session counters for each event type.
- **Class legend** — live population count and average level per class.
- **Boss-kill banner** — a full-screen dramatic overlay for the rare occasions a group actually clears an encounter.

## Architecture

```
config.toml           # DB connection + poll interval (gitignored, real credentials)
tools/extract_dbc.rs   # one-time: pulls real achievement/zone/map names from your WoW client
data/*.json            # output of the above, loaded at startup
src/
  main.rs              # axum HTTP + WebSocket server, static file serving
  config.rs            # config.toml loading
  db.rs                # sqlx queries against the AzerothCore database
  poller.rs            # polls on a timer, diffs against in-memory state, synthesizes events
  events.rs            # the Event enum + RosterEntry/RosterMessage wire types
  names.rs             # achievement/zone/map ID -> name lookups
  character_info.rs     # race/class ID -> name
web/
  index.html, style.css, app.js   # the frontend: Matrix rain, agent field, ticker, stats, legend
```

The backend polls the database every `poll_interval_secs`, diffs against its own in-memory snapshot of the population, and broadcasts two kinds of WebSocket messages: individual `Event`s (for the ticker) and a full `RosterMessage` (the complete online population, for the agent field and stats — sent every tick, not just on change, so a newly-connected browser doesn't wait for a delta to draw anything).

No frontend framework — vanilla JS driving the DOM and two `<canvas>` layers (Matrix rain, and per-continent radar/link-line drawing).

## Setup

**1. Create a read-only database user** (skip if you already made one for wow-gm-console — this reuses it):

```sql
CREATE USER IF NOT EXISTS 'gmconsole_ro'@'%' IDENTIFIED BY 'CHOOSE-A-STRONG-PASSWORD';
GRANT SELECT ON acore_world.* TO 'gmconsole_ro'@'%';
GRANT SELECT ON acore_characters.* TO 'gmconsole_ro'@'%';
FLUSH PRIVILEGES;
```

**2. Configure:**

```bash
cp config.toml.example config.toml
```

Edit `config.toml` with your real host/port/credentials.

**3. Extract real display names from your WoW 3.3.5a client** (achievements, zones, continent/instance maps — one-time, only needs re-running if you point at a different client):

```bash
cargo run --bin extract_dbc -- /path/to/your/WoWClient
```

This writes `data/achievement_names.json`, `data/zone_names.json`, and `data/map_names.json`. If you skip this, everything falls back to showing raw IDs (`Achievement #1234`) instead of real names.

**4. Run it:**

For development:

```bash
cargo run --bin overseer
```

Open `http://localhost:7777` (or whatever `http_port` you set) in a browser and go full-screen (F11).

For an actual presentation, use the launch script instead — it builds a release binary, starts the server, waits for it to come up, and opens a dedicated kiosk-mode browser window (no address bar, no tabs, no accidental navigation):

```bash
./launch.sh
```

It picks up whichever of Chrome/Chromium/Firefox is on your `PATH` (Chrome/Chromium preferred). Ctrl+C in the terminal running it stops the server too. If something's already listening on the configured port, it assumes the server is already running and just opens the browser, leaving that process alone.

## Making the event feed livelier

Several event types are gated by how often AzerothCore actually writes character state to the database, **not** by how often this tool polls. By default, `worldserver.conf`'s `PlayerSaveInterval` is 900000ms (15 minutes) — meaning a bot's zone/level/position can sit stale in the DB for up to 15 minutes after it actually changes. For a livelier demo, lower it and enable additional-save triggers:

```
PlayerSaveInterval = 30000
PlayerSave.AdditionalSaves = 7
```

(`AdditionalSaves = 7` triggers an early save a few seconds after inventory/quest/achievement changes, independent of the main interval.) Restart the worldserver after changing this. Confirmed live: this took zone-change visibility from ~1 per 90 seconds across 500 bots to ~15 per 90 seconds — the underlying movement was always happening, it just wasn't reaching the database promptly.

## Event types and their real data sources

| Event | Source | Notes |
|---|---|---|
| Went online/offline, level up, zone change, death | `characters` table, diffed each poll | Death = health hits 0 (the `death_expire_time` column looked promising but proved unreliable — most alive characters have it nonzero) |
| Achievement earned | `character_achievement` | |
| Guild activity | `guild_eventlog` | EventType-to-description mapping is best-effort, not yet cross-checked against real guild activity |
| Group formed/disbanded | `group_member` / `groups` | |
| Quest completed | `character_queststatus_rewarded` joined against `quest_template` | No timestamp column on this table — tracked via an in-memory per-bot known-quest-ID diff instead |
| Notable item found (quality ≥ uncommon) | `character_inventory` + `item_instance` + `item_template` | Same no-timestamp situation as quests, same diffing approach. Excludes stackable items (`stackable > 1`) — a stackable item's row gets recreated whenever its stack reshuffles, which looks like "found it again" if you don't filter that out |
| Gathering skill-up (Mining/Herbalism/Skinning) | `character_skills` | Signals "successfully gathered recently," not a per-node log — AzerothCore doesn't keep one, so a skill-up is the best real proxy, the same relationship a level-up has to killing mobs |
| Boss kill | `log_encounter` joined against `creature_template` | Genuinely rare: Playerbots groups clear dungeons/raids far less often than they solo-quest. Structurally correct but may be silent for an entire session unless a group actually finishes an encounter |
| Chat exchange | `mod_ollama_chat_history` | Only populates if a real player whispers a bot; treat as a bonus channel to trigger live on stage, not a steady source |

## Geographic bot placement

Each bot's dot is placed using AzerothCore's own world-to-grid math (`SIZE_OF_GRIDS = 533.3333`, a 64×64 grid per continent — taken directly from the engine source, not guessed), so positions are geographically real relative to each other. The backdrop is a stylized radar grid rather than real map art — no continent map images are bundled or extracted in this version.

## Known limitations

- No real map imagery — see above.
- Boss-kill and chat channels can be silent for an entire presentation depending on what the bots are actually doing; don't rely on them as the backbone of a demo.
- Single-server, single-profile by design (unlike wow-gm-console) — this is a presentation tool for one server, not a multi-server admin console.

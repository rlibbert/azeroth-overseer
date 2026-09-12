use std::collections::HashMap;
use std::fs;

pub struct NameTables {
    achievements: HashMap<u32, String>,
    zones: HashMap<u32, String>,
}

impl NameTables {
    pub fn load() -> Self {
        let achievements = Self::load_map("data/achievement_names.json");
        let zones = Self::load_map("data/zone_names.json");
        println!(
            "Loaded {} achievement names and {} zone names",
            achievements.len(),
            zones.len()
        );
        Self { achievements, zones }
    }

    fn load_map(path: &str) -> HashMap<u32, String> {
        let raw = fs::read_to_string(path).unwrap_or_else(|_| {
            eprintln!(
                "warning: {path} not found -- run `cargo run --bin extract_dbc -- /path/to/WoWClient` first. Names will show as raw IDs."
            );
            "{}".to_string()
        });
        // Extracted JSON keys are strings (JSON object keys are always
        // strings); parse into a string-keyed map first, then convert.
        let raw_map: HashMap<String, String> = serde_json::from_str(&raw).unwrap_or_default();
        raw_map
            .into_iter()
            .filter_map(|(k, v)| k.parse::<u32>().ok().map(|id| (id, v)))
            .collect()
    }

    pub fn achievement_name(&self, id: u32) -> String {
        self.achievements
            .get(&id)
            .cloned()
            .unwrap_or_else(|| format!("Achievement #{id}"))
    }

    pub fn zone_name(&self, id: u32) -> String {
        self.zones.get(&id).cloned().unwrap_or_else(|| format!("Zone #{id}"))
    }
}

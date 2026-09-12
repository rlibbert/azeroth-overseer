//! One-time local utility: extracts real achievement and zone names from
//! the user's own WoW 3.3.5a client MPQ archives, so the presentation
//! console never has to guess/hallucinate display text. Run once:
//!
//!   cargo run --bin extract_dbc -- /path/to/WoWClient
//!
//! Writes data/achievement_names.json and data/zone_names.json.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use wow_dbc::wrath_tables::{achievement, area_table};
use wow_dbc::DbcTable;

/// MPQs searched in override order (later entries win if a file exists in
/// multiple archives) -- DBC files carrying localized strings normally live
/// in the locale-specific archives, patched forward by the numbered patches.
const MPQ_SEARCH_ORDER: &[&str] = &[
    "Data/common.MPQ",
    "Data/expansion.MPQ",
    "Data/lichking.MPQ",
    "Data/patch.MPQ",
    "Data/patch-2.MPQ",
    "Data/patch-3.MPQ",
    "Data/enUS/locale-enUS.MPQ",
    "Data/enUS/patch-enUS.MPQ",
    "Data/enUS/patch-enUS-2.MPQ",
    "Data/enUS/patch-enUS-3.MPQ",
];

fn find_dbc_bytes(client_root: &Path, dbc_name: &str) -> Option<Vec<u8>> {
    let want = format!("DBFilesClient\\{dbc_name}");
    let mut found = None;

    for rel in MPQ_SEARCH_ORDER {
        let mpq_path = client_root.join(rel);
        if !mpq_path.exists() {
            continue;
        }
        match wow_mpq::Archive::open(&mpq_path) {
            Ok(mut archive) => {
                if let Ok(data) = archive.read_file(&want) {
                    println!("  found {dbc_name} in {rel} ({} bytes)", data.len());
                    found = Some(data);
                }
            }
            Err(e) => {
                eprintln!("  warning: could not open {rel}: {e}");
            }
        }
    }

    found
}

fn main() {
    let client_root: PathBuf = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/home/rl/Downloads/WoWClient"));

    println!("Reading client data from {}", client_root.display());

    println!("Extracting Achievement.dbc...");
    let achievement_bytes =
        find_dbc_bytes(&client_root, "Achievement.dbc").expect("Achievement.dbc not found in any searched MPQ");
    let achievement_table =
        achievement::Achievement::read(&mut achievement_bytes.as_slice()).expect("failed to parse Achievement.dbc");

    let mut achievement_names: HashMap<u32, String> = HashMap::new();
    for row in achievement_table.rows() {
        let name = row.title_lang.en_gb.clone();
        if !name.is_empty() {
            achievement_names.insert(row.id.id as u32, name);
        }
    }
    println!("  parsed {} named achievements", achievement_names.len());

    println!("Extracting AreaTable.dbc...");
    let area_bytes =
        find_dbc_bytes(&client_root, "AreaTable.dbc").expect("AreaTable.dbc not found in any searched MPQ");
    let area_table_data =
        area_table::AreaTable::read(&mut area_bytes.as_slice()).expect("failed to parse AreaTable.dbc");

    let mut zone_names: HashMap<u32, String> = HashMap::new();
    for row in area_table_data.rows() {
        let name = row.area_name_lang.en_gb.clone();
        if !name.is_empty() {
            zone_names.insert(row.id.id as u32, name);
        }
    }
    println!("  parsed {} named zones", zone_names.len());

    fs::create_dir_all("data").expect("failed to create data/ directory");
    fs::write(
        "data/achievement_names.json",
        serde_json::to_string_pretty(&achievement_names).unwrap(),
    )
    .expect("failed to write achievement_names.json");
    fs::write(
        "data/zone_names.json",
        serde_json::to_string_pretty(&zone_names).unwrap(),
    )
    .expect("failed to write zone_names.json");

    println!("Wrote data/achievement_names.json and data/zone_names.json");
}

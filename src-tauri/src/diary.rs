//! Versioned diary file.
//!
//! The file is JSON, not a copy of the live SQLite database. Import writes
//! through the current schema, so a later migration that adds a column with a
//! default still accepts a version 1 file. Extra JSON fields are ignored.
//! Bump `VERSION` when an old build must refuse the file.
//!
//! USDA `food_cache` rows and the API key (stored by the store plugin, not in
//! these tables) are intentionally absent.

use chrono::{NaiveDate, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::db::{self, CustomFood, Goal};
use crate::error::{AppError, Result};

pub const FORMAT: &str = "nomnom-nanny-diary";
pub const VERSION: u32 = 1;
const MAX_BYTES: usize = 20 * 1024 * 1024;
const NAME_PREVIEW_LIMIT: usize = 8;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiaryFile {
    pub format: String,
    pub version: u32,
    #[serde(default)]
    pub exported_at: Option<String>,
    pub goals: Goal,
    pub entries: Vec<DiaryEntry>,
    pub custom_foods: Vec<DiaryCustomFood>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiaryEntry {
    pub log_date: String,
    #[serde(default)]
    pub fdc_id: Option<i64>,
    pub description: String,
    pub amount: f64,
    pub unit: String,
    #[serde(default)]
    pub grams: Option<f64>,
    pub calories_kcal: f64,
    pub protein_g: f64,
    pub fat_g: f64,
    pub carbs_g: f64,
    pub fiber_g: f64,
    pub fluid_oz: f64,
    #[serde(default)]
    pub meal: Option<String>,
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default)]
    pub created_at: Option<String>,
}

fn default_source() -> String {
    "custom".into()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiaryCustomFood {
    pub name: String,
    pub kcal_per_100g: f64,
    pub protein_per_100g: f64,
    pub fat_per_100g: f64,
    pub carbs_per_100g: f64,
    pub fiber_per_100g: f64,
}

#[derive(Debug, Serialize, Clone)]
pub struct DiarySummary {
    pub entry_count: usize,
    pub first_date: Option<String>,
    pub last_date: Option<String>,
    pub custom_food_count: usize,
    pub custom_food_names: Vec<String>,
    pub custom_foods_omitted: usize,
    pub goals: Goal,
}

#[derive(Debug, Serialize, Clone)]
pub struct DiaryImportPreview {
    pub device: DiarySummary,
    pub file: DiarySummary,
}

pub fn export_diary(conn: &Connection) -> Result<String> {
    let file = DiaryFile {
        format: FORMAT.into(),
        version: VERSION,
        exported_at: Some(Utc::now().to_rfc3339()),
        goals: db::get_goals(conn)?,
        entries: load_entries(conn)?,
        custom_foods: load_custom_foods(conn)?,
    };
    Ok(serde_json::to_string_pretty(&file)? + "\n")
}

pub fn preview_diary(conn: &Connection, contents: &str) -> Result<DiaryImportPreview> {
    let file = parse_diary(contents)?;
    Ok(DiaryImportPreview {
        device: summarize_device(conn)?,
        file: summarize_file(&file),
    })
}

/// Replace goals, intake entries, and custom foods.
///
/// The USDA cache is left untouched. Callers must already have shown the
/// preview and received an explicit confirmation.
pub fn import_diary(conn: &Connection, contents: &str) -> Result<()> {
    let file = parse_diary(contents)?;
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM intake_entries", [])?;
    tx.execute("DELETE FROM custom_foods", [])?;
    upsert_goals(&tx, &file.goals)?;
    for entry in &file.entries {
        insert_entry(&tx, entry)?;
    }
    for food in &file.custom_foods {
        insert_custom_food(&tx, food)?;
    }
    tx.commit()?;
    Ok(())
}

#[derive(Deserialize)]
struct DiaryHeader {
    format: String,
    version: u32,
}

fn parse_diary(contents: &str) -> Result<DiaryFile> {
    let contents = contents.trim().trim_start_matches('\u{feff}');
    if contents.len() > MAX_BYTES {
        return Err(AppError::InvalidInput(
            "That file is too large to be a NomNom diary.".into(),
        ));
    }
    let not_a_diary = || {
        AppError::InvalidInput(
            "This file is not a NomNom diary. Export one from NomNom Nanny and choose that file."
                .into(),
        )
    };
    let header: DiaryHeader = serde_json::from_str(contents).map_err(|_| not_a_diary())?;
    if header.format != FORMAT {
        return Err(not_a_diary());
    }
    if header.version == 0 || header.version > VERSION {
        return Err(AppError::InvalidInput(
            "This diary was exported by a newer NomNom Nanny. Update the app before importing it."
                .into(),
        ));
    }
    if header.version != VERSION {
        return Err(AppError::InvalidInput(
            "This diary file uses a version NomNom Nanny does not read.".into(),
        ));
    }
    let file: DiaryFile = serde_json::from_str(contents).map_err(|_| not_a_diary())?;
    validate(&file)?;
    Ok(file)
}

fn validate(file: &DiaryFile) -> Result<()> {
    if file.format != FORMAT {
        return Err(AppError::InvalidInput(
            "This file is not a NomNom diary.".into(),
        ));
    }
    if file.version == 0 || file.version > VERSION {
        return Err(AppError::InvalidInput(
            "This diary was exported by a newer NomNom Nanny. Update the app before importing it."
                .into(),
        ));
    }
    if file.version != VERSION {
        return Err(AppError::InvalidInput(
            "This diary file uses a version NomNom Nanny does not read.".into(),
        ));
    }
    finite_goal(&file.goals)?;
    for entry in &file.entries {
        if NaiveDate::parse_from_str(&entry.log_date, "%Y-%m-%d").is_err() {
            return Err(AppError::InvalidDate(entry.log_date.clone()));
        }
        finite("amount", entry.amount)?;
        finite("calories", entry.calories_kcal)?;
        finite("protein", entry.protein_g)?;
        finite("fat", entry.fat_g)?;
        finite("carbs", entry.carbs_g)?;
        finite("fiber", entry.fiber_g)?;
        finite("water", entry.fluid_oz)?;
        if let Some(grams) = entry.grams {
            finite("grams", grams)?;
        }
        if entry.unit.trim().is_empty() {
            return Err(AppError::InvalidInput(
                "A log entry is missing its unit.".into(),
            ));
        }
    }
    for food in &file.custom_foods {
        if food.name.trim().is_empty() {
            return Err(AppError::InvalidInput(
                "A custom food is missing its name.".into(),
            ));
        }
        finite("custom food calories", food.kcal_per_100g)?;
        finite("custom food protein", food.protein_per_100g)?;
        finite("custom food fat", food.fat_per_100g)?;
        finite("custom food carbs", food.carbs_per_100g)?;
        finite("custom food fiber", food.fiber_per_100g)?;
    }
    Ok(())
}

fn finite_goal(goal: &Goal) -> Result<()> {
    finite("calorie goal", goal.calories_kcal)?;
    finite("protein goal", goal.protein_g)?;
    finite("fat goal", goal.fat_g)?;
    finite("carb goal", goal.carbs_g)?;
    finite("fiber goal", goal.fiber_g)?;
    finite("water goal", goal.hydration_oz)?;
    Ok(())
}

fn finite(label: &str, value: f64) -> Result<()> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(AppError::InvalidInput(format!(
            "The diary has a {label} value that is not a number."
        )))
    }
}

fn summarize_device(conn: &Connection) -> Result<DiarySummary> {
    let goals = db::get_goals(conn)?;
    let entries = load_entries(conn)?;
    let foods = load_custom_foods(conn)?;
    Ok(summarize(&goals, &entries, &foods))
}

fn summarize_file(file: &DiaryFile) -> DiarySummary {
    summarize(&file.goals, &file.entries, &file.custom_foods)
}

fn summarize(goals: &Goal, entries: &[DiaryEntry], foods: &[DiaryCustomFood]) -> DiarySummary {
    let mut dates: Vec<&str> = entries
        .iter()
        .map(|entry| entry.log_date.as_str())
        .collect();
    dates.sort_unstable();
    let mut names: Vec<String> = foods.iter().map(|food| food.name.clone()).collect();
    names.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    let omitted = names.len().saturating_sub(NAME_PREVIEW_LIMIT);
    names.truncate(NAME_PREVIEW_LIMIT);
    DiarySummary {
        entry_count: entries.len(),
        first_date: dates.first().map(|date| (*date).to_string()),
        last_date: dates.last().map(|date| (*date).to_string()),
        custom_food_count: foods.len(),
        custom_food_names: names,
        custom_foods_omitted: omitted,
        goals: goals.clone(),
    }
}

fn load_entries(conn: &Connection) -> Result<Vec<DiaryEntry>> {
    let mut stmt = conn.prepare(
        "SELECT log_date, fdc_id, description, amount, unit, grams,
                calories_kcal, protein_g, fat_g, carbs_g, fiber_g, fluid_oz,
                meal, source, created_at
         FROM intake_entries
         ORDER BY log_date ASC, created_at ASC, id ASC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(DiaryEntry {
                log_date: row.get(0)?,
                fdc_id: row.get(1)?,
                description: row.get(2)?,
                amount: row.get(3)?,
                unit: row.get(4)?,
                grams: row.get(5)?,
                calories_kcal: row.get(6)?,
                protein_g: row.get(7)?,
                fat_g: row.get(8)?,
                carbs_g: row.get(9)?,
                fiber_g: row.get(10)?,
                fluid_oz: row.get(11)?,
                meal: row.get(12)?,
                source: row.get(13)?,
                created_at: row.get(14)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_custom_foods(conn: &Connection) -> Result<Vec<DiaryCustomFood>> {
    let foods = db::get_custom_foods(conn)?;
    Ok(foods.into_iter().map(custom_food_to_diary).collect())
}

fn custom_food_to_diary(food: CustomFood) -> DiaryCustomFood {
    DiaryCustomFood {
        name: food.name,
        kcal_per_100g: food.kcal_per_100g,
        protein_per_100g: food.protein_per_100g,
        fat_per_100g: food.fat_per_100g,
        carbs_per_100g: food.carbs_per_100g,
        fiber_per_100g: food.fiber_per_100g,
    }
}

fn upsert_goals(conn: &Connection, goal: &Goal) -> Result<()> {
    let changed = conn.execute(
        "UPDATE goals SET
            calories_kcal = ?1,
            protein_g = ?2,
            fat_g = ?3,
            carbs_g = ?4,
            fiber_g = ?5,
            hydration_oz = ?6,
            updated_at = CURRENT_TIMESTAMP
         WHERE id = 1",
        params![
            goal.calories_kcal,
            goal.protein_g,
            goal.fat_g,
            goal.carbs_g,
            goal.fiber_g,
            goal.hydration_oz,
        ],
    )?;
    if changed == 0 {
        conn.execute(
            "INSERT INTO goals (id, calories_kcal, protein_g, fat_g, carbs_g, fiber_g, hydration_oz)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                goal.calories_kcal,
                goal.protein_g,
                goal.fat_g,
                goal.carbs_g,
                goal.fiber_g,
                goal.hydration_oz,
            ],
        )?;
    }
    Ok(())
}

fn insert_entry(conn: &Connection, entry: &DiaryEntry) -> Result<()> {
    let source = if entry.source.trim().is_empty() {
        "custom"
    } else {
        entry.source.as_str()
    };
    if let Some(created_at) = entry.created_at.as_deref() {
        conn.execute(
            "INSERT INTO intake_entries (
                log_date, fdc_id, description, amount, unit, grams,
                calories_kcal, protein_g, fat_g, carbs_g, fiber_g, fluid_oz,
                meal, source, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                entry.log_date,
                entry.fdc_id,
                entry.description,
                entry.amount,
                entry.unit,
                entry.grams,
                entry.calories_kcal,
                entry.protein_g,
                entry.fat_g,
                entry.carbs_g,
                entry.fiber_g,
                entry.fluid_oz,
                entry.meal,
                source,
                created_at,
            ],
        )?;
    } else {
        conn.execute(
            "INSERT INTO intake_entries (
                log_date, fdc_id, description, amount, unit, grams,
                calories_kcal, protein_g, fat_g, carbs_g, fiber_g, fluid_oz,
                meal, source
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                entry.log_date,
                entry.fdc_id,
                entry.description,
                entry.amount,
                entry.unit,
                entry.grams,
                entry.calories_kcal,
                entry.protein_g,
                entry.fat_g,
                entry.carbs_g,
                entry.fiber_g,
                entry.fluid_oz,
                entry.meal,
                source,
            ],
        )?;
    }
    Ok(())
}

fn insert_custom_food(conn: &Connection, food: &DiaryCustomFood) -> Result<()> {
    db::save_custom_food(
        conn,
        &CustomFood {
            id: 0,
            name: food.name.clone(),
            kcal_per_100g: food.kcal_per_100g,
            protein_per_100g: food.protein_per_100g,
            fat_per_100g: food.fat_per_100g,
            carbs_per_100g: food.carbs_per_100g,
            fiber_per_100g: food.fiber_per_100g,
        },
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{init_db, log_intake, save_custom_food, update_goals, LogEntryInput};
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_DIRS: AtomicU64 = AtomicU64::new(0);

    struct TempDb {
        dir: std::path::PathBuf,
        state: crate::db::DbState,
    }

    impl TempDb {
        fn new() -> Self {
            let n = TEMP_DIRS.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "nomnom-diary-{}-{}-{}",
                std::process::id(),
                n,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let state = init_db(dir.clone()).unwrap();
            Self { dir, state }
        }

        fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
            self.state.lock().unwrap()
        }
    }

    impl Drop for TempDb {
        fn drop(&mut self) {
            let state = std::mem::replace(
                &mut self.state,
                std::sync::Arc::new(std::sync::Mutex::new(Connection::open_in_memory().unwrap())),
            );
            drop(state);
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn sample_goal() -> Goal {
        Goal {
            calories_kcal: 1800.0,
            protein_g: 120.0,
            fat_g: 60.0,
            carbs_g: 180.0,
            fiber_g: 28.0,
            hydration_oz: 80.0,
        }
    }

    fn seed(db: &TempDb) {
        let conn = db.conn();
        update_goals(&conn, &sample_goal()).unwrap();
        log_intake(
            &conn,
            &LogEntryInput {
                log_date: "2026-10-03".into(),
                fdc_id: Some(171077),
                description: "Banana".into(),
                amount: 1.0,
                unit: "medium".into(),
                grams: Some(118.0),
                calories_kcal: 105.0,
                protein_g: 1.3,
                fat_g: 0.4,
                carbs_g: 27.0,
                fiber_g: 3.1,
                fluid_oz: 0.0,
                meal: Some("Breakfast".into()),
                source: Some("usda".into()),
            },
        )
        .unwrap();
        log_intake(
            &conn,
            &LogEntryInput {
                log_date: "2026-10-04".into(),
                fdc_id: None,
                description: "Water".into(),
                amount: 12.0,
                unit: "fl oz".into(),
                grams: None,
                calories_kcal: 0.0,
                protein_g: 0.0,
                fat_g: 0.0,
                carbs_g: 0.0,
                fiber_g: 0.0,
                fluid_oz: 12.0,
                meal: None,
                source: Some("water".into()),
            },
        )
        .unwrap();
        conn.execute(
            "UPDATE intake_entries SET created_at = '2026-10-03 08:00:00' WHERE description = 'Banana'",
            [],
        )
        .unwrap();
        save_custom_food(
            &conn,
            &CustomFood {
                id: 0,
                name: "Chicken salad".into(),
                kcal_per_100g: 180.0,
                protein_per_100g: 20.0,
                fat_per_100g: 8.0,
                carbs_per_100g: 4.0,
                fiber_per_100g: 1.0,
            },
        )
        .unwrap();
        conn.execute(
            "INSERT INTO food_cache (fdc_id, description, nutrients_json, portions_json)
             VALUES (171077, 'Bananas, raw', '{}', '[]')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn export_round_trips_diary_and_leaves_usda_cache() {
        let db = TempDb::new();
        seed(&db);
        let exported = export_diary(&db.conn()).unwrap();
        assert!(!exported.contains("usdaApiKey"));
        assert!(!exported.contains("DEMO_KEY"));
        assert!(!exported.contains("food_cache"));
        assert!(!exported.contains("nutrients_json"));
        assert!(exported.contains("\"format\": \"nomnom-nanny-diary\""));

        let parsed: DiaryFile = serde_json::from_str(&exported).unwrap();
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(parsed.entries[0].description, "Banana");
        assert_eq!(parsed.entries[0].fdc_id, Some(171077));
        assert_eq!(
            parsed.entries[0].created_at.as_deref(),
            Some("2026-10-03 08:00:00")
        );
        assert_eq!(parsed.entries[1].description, "Water");
        assert_eq!(parsed.custom_foods.len(), 1);
        assert!((parsed.goals.hydration_oz - 80.0).abs() < f64::EPSILON);

        {
            let conn = db.conn();
            update_goals(
                &conn,
                &Goal {
                    calories_kcal: 2000.0,
                    protein_g: 10.0,
                    fat_g: 10.0,
                    carbs_g: 10.0,
                    fiber_g: 10.0,
                    hydration_oz: 10.0,
                },
            )
            .unwrap();
            conn.execute("DELETE FROM intake_entries", []).unwrap();
            conn.execute("DELETE FROM custom_foods", []).unwrap();
            log_intake(
                &conn,
                &LogEntryInput {
                    log_date: "2026-01-01".into(),
                    fdc_id: None,
                    description: "Old toast".into(),
                    amount: 1.0,
                    unit: "slice".into(),
                    grams: None,
                    calories_kcal: 70.0,
                    protein_g: 2.0,
                    fat_g: 1.0,
                    carbs_g: 13.0,
                    fiber_g: 1.0,
                    fluid_oz: 0.0,
                    meal: None,
                    source: Some("custom".into()),
                },
            )
            .unwrap();
        }

        let preview = preview_diary(&db.conn(), &exported).unwrap();
        assert_eq!(preview.device.entry_count, 1);
        assert_eq!(preview.device.first_date.as_deref(), Some("2026-01-01"));
        assert_eq!(preview.file.entry_count, 2);
        assert_eq!(preview.file.first_date.as_deref(), Some("2026-10-03"));
        assert_eq!(preview.file.last_date.as_deref(), Some("2026-10-04"));
        assert_eq!(
            preview.file.custom_food_names,
            vec!["Chicken salad".to_string()]
        );
        assert_eq!(
            db.conn()
                .query_row("SELECT COUNT(*) FROM intake_entries", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );

        import_diary(&db.conn(), &exported).unwrap();
        let again = export_diary(&db.conn()).unwrap();
        let round: DiaryFile = serde_json::from_str(&again).unwrap();
        assert_eq!(round.goals.calories_kcal, 1800.0);
        assert_eq!(round.entries.len(), 2);
        assert_eq!(round.entries[0].description, "Banana");
        assert_eq!(
            round.entries[0].created_at.as_deref(),
            Some("2026-10-03 08:00:00")
        );
        assert_eq!(round.entries[1].fluid_oz, 12.0);
        assert_eq!(round.custom_foods[0].name, "Chicken salad");
        assert_eq!(round.custom_foods[0].protein_per_100g, 20.0);
        let cache: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM food_cache WHERE fdc_id = 171077",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(cache, 1);
    }

    #[test]
    fn rejected_import_does_not_change_the_open_database() {
        let db = TempDb::new();
        seed(&db);
        let before = export_diary(&db.conn()).unwrap();
        let err = import_diary(
            &db.conn(),
            "{\"format\":\"nomnom-nanny-diary\",\"version\":2}",
        )
        .unwrap_err();
        assert!(err.to_string().contains("newer"));
        assert_eq!(
            export_diary(&db.conn()).unwrap().contains("Banana"),
            before.contains("Banana")
        );
        let err = import_diary(&db.conn(), "not json").unwrap_err();
        assert!(err.to_string().contains("not a NomNom diary"));
        let goals = db::get_goals(&db.conn()).unwrap();
        assert_eq!(goals.calories_kcal, 1800.0);
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM intake_entries", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn version_one_accepts_unknown_fields_and_optional_gaps() {
        let db = TempDb::new();
        let contents = r#"{
            "format": "nomnom-nanny-diary",
            "version": 1,
            "future_note": "ignored",
            "goals": {
                "calories_kcal": 1900,
                "protein_g": 100,
                "fat_g": 55,
                "carbs_g": 200,
                "fiber_g": 25,
                "hydration_oz": 70,
                "future_goal": 1
            },
            "entries": [{
                "log_date": "2026-05-01",
                "description": "Oats",
                "amount": 40,
                "unit": "g",
                "calories_kcal": 150,
                "protein_g": 5,
                "fat_g": 3,
                "carbs_g": 27,
                "fiber_g": 4,
                "fluid_oz": 0,
                "extra": true
            }],
            "custom_foods": []
        }"#;
        import_diary(&db.conn(), contents).unwrap();
        let goals = db::get_goals(&db.conn()).unwrap();
        assert_eq!(goals.calories_kcal, 1900.0);
        let entry = db::get_entries_for_date(&db.conn(), "2026-05-01").unwrap();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].grams, None);
        assert_eq!(entry[0].source, "custom");
        assert_eq!(entry[0].fdc_id, None);
    }
}

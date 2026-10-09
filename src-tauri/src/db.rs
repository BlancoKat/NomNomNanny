use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension, Result as SqliteResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::error::{AppError, Result};

/// Shared database handle stored in Tauri app state.
pub type DbState = Arc<Mutex<Connection>>;

/// Open (or create) the SQLite DB in the app data directory.
/// Creates parent dirs and runs migrations.
pub fn init_db(app_data_dir: PathBuf) -> Result<DbState> {
    std::fs::create_dir_all(&app_data_dir)?;
    let db_path = app_data_dir.join("nomnom_nanny.db");

    let conn = Connection::open(&db_path)?;

    // Good pragmas for desktop app
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;
        PRAGMA busy_timeout = 5000;
        "#,
    )?;

    run_migrations(&conn)?;

    Ok(Arc::new(Mutex::new(conn)))
}

fn run_migrations(conn: &Connection) -> SqliteResult<()> {
    // Simple migration tracking table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    let current_version: i32 = conn
        .query_row("SELECT COALESCE(MAX(version), 0) FROM schema_migrations", [], |r| r.get(0))
        .unwrap_or(0);

    // --- Migration 1: initial schema ---
    if current_version < 1 {
        conn.execute_batch(
            r#"
            -- Goals (single row, id = 1)
            CREATE TABLE IF NOT EXISTS goals (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                calories_kcal REAL NOT NULL DEFAULT 2000,
                protein_g     REAL NOT NULL DEFAULT 150,
                fat_g         REAL NOT NULL DEFAULT 65,
                carbs_g       REAL NOT NULL DEFAULT 225,
                fiber_g       REAL NOT NULL DEFAULT 30,
                hydration_oz  REAL NOT NULL DEFAULT 64,
                updated_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            -- USDA food cache
            CREATE TABLE IF NOT EXISTS food_cache (
                fdc_id          INTEGER PRIMARY KEY,
                description     TEXT NOT NULL,
                data_type       TEXT,
                brand_owner     TEXT,
                nutrients_json  TEXT NOT NULL,
                portions_json   TEXT NOT NULL,
                cached_at       TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            -- All intake entries (food, custom, water)
            CREATE TABLE IF NOT EXISTS intake_entries (
                id             INTEGER PRIMARY KEY AUTOINCREMENT,
                log_date       TEXT NOT NULL,           -- '2026-05-28'
                fdc_id         INTEGER,
                description    TEXT NOT NULL,
                amount         REAL NOT NULL,
                unit           TEXT NOT NULL,
                grams          REAL,
                calories_kcal  REAL NOT NULL DEFAULT 0,
                protein_g      REAL NOT NULL DEFAULT 0,
                fat_g          REAL NOT NULL DEFAULT 0,
                carbs_g        REAL NOT NULL DEFAULT 0,
                fiber_g        REAL NOT NULL DEFAULT 0,
                fluid_oz       REAL NOT NULL DEFAULT 0,
                meal           TEXT,
                source         TEXT NOT NULL DEFAULT 'usda',
                created_at     TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at     TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_intake_date ON intake_entries(log_date);
            CREATE INDEX IF NOT EXISTS idx_intake_fdc  ON intake_entries(fdc_id);

            -- Seed default goals
            INSERT OR IGNORE INTO goals (id) VALUES (1);
            "#,
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO schema_migrations (version) VALUES (1)",
            [],
        )?;
    }

    // --- Migration 2: custom foods (per-100g base for reusable manual entries) ---
    if current_version < 2 {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS custom_foods (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                kcal_per_100g     REAL NOT NULL DEFAULT 0,
                protein_per_100g  REAL NOT NULL DEFAULT 0,
                fat_per_100g      REAL NOT NULL DEFAULT 0,
                carbs_per_100g    REAL NOT NULL DEFAULT 0,
                fiber_per_100g    REAL NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_custom_foods_name ON custom_foods(name);
            "#,
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO schema_migrations (version) VALUES (2)",
            [],
        )?;
    }

    // --- Migration 3: serving unit and optional density on custom foods ---
    // Existing rows are gram foods (basis_unit 'g', density NULL = 1 g/mL).
    if current_version < 3 {
        conn.execute_batch(
            r#"
            ALTER TABLE custom_foods ADD COLUMN basis_unit TEXT NOT NULL DEFAULT 'g';
            ALTER TABLE custom_foods ADD COLUMN density_g_per_ml REAL;
            "#,
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO schema_migrations (version) VALUES (3)",
            [],
        )?;
    }

    Ok(())
}

// ---------------------- DATA MODELS ----------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Goal {
    pub calories_kcal: f64,
    pub protein_g: f64,
    pub fat_g: f64,
    pub carbs_g: f64,
    pub fiber_g: f64,
    pub hydration_oz: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IntakeEntry {
    pub id: i64,
    pub log_date: String,
    pub fdc_id: Option<i64>,
    pub description: String,
    pub amount: f64,
    pub unit: String,
    pub grams: Option<f64>,
    pub calories_kcal: f64,
    pub protein_g: f64,
    pub fat_g: f64,
    pub carbs_g: f64,
    pub fiber_g: f64,
    pub fluid_oz: f64,
    pub meal: Option<String>,
    pub source: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomFood {
    pub id: i64,
    pub name: String,
    pub kcal_per_100g: f64,
    pub protein_per_100g: f64,
    pub fat_per_100g: f64,
    pub carbs_per_100g: f64,
    pub fiber_per_100g: f64,
    /// Unit the label was entered in: g, ml, oz, or fl oz. Older rows are g.
    #[serde(default = "default_basis_unit")]
    pub basis_unit: String,
    /// Grams per mL. NULL means the default, 1 g/mL.
    #[serde(default)]
    pub density_g_per_ml: Option<f64>,
}

fn default_basis_unit() -> String {
    "g".to_string()
}

/// Nutrients as printed on a label, for `amount` of `unit`.
#[derive(Debug, Deserialize)]
pub struct CustomFoodLabel {
    pub name: String,
    pub amount: f64,
    pub unit: String,
    #[serde(default)]
    pub density_g_per_ml: Option<f64>,
    pub kcal: f64,
    pub protein: f64,
    pub fat: f64,
    pub carbs: f64,
    pub fiber: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DailyTotals {
    pub log_date: String,
    pub calories_kcal: f64,
    pub protein_g: f64,
    pub fat_g: f64,
    pub carbs_g: f64,
    pub fiber_g: f64,
    pub fluid_oz: f64,
    pub entry_count: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Averages {
    pub days: i32,
    pub calories_kcal: f64,
    pub protein_g: f64,
    pub fat_g: f64,
    pub carbs_g: f64,
    pub fiber_g: f64,
    pub fluid_oz: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HistoryDay {
    pub log_date: String,
    pub calories_kcal: f64,
    pub protein_g: f64,
    pub fat_g: f64,
    pub carbs_g: f64,
    pub fiber_g: f64,
    pub fluid_oz: f64,
    pub entry_count: i64,
}

// ---------------------- GOALS ----------------------

pub fn get_goals(conn: &Connection) -> Result<Goal> {
    let mut stmt = conn.prepare(
        "SELECT calories_kcal, protein_g, fat_g, carbs_g, fiber_g, hydration_oz FROM goals WHERE id = 1",
    )?;
    let goal = stmt.query_row([], |row| {
        Ok(Goal {
            calories_kcal: row.get(0)?,
            protein_g: row.get(1)?,
            fat_g: row.get(2)?,
            carbs_g: row.get(3)?,
            fiber_g: row.get(4)?,
            hydration_oz: row.get(5)?,
        })
    })?;
    Ok(goal)
}

pub fn update_goals(conn: &Connection, goal: &Goal) -> Result<()> {
    conn.execute(
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
    Ok(())
}

// ---------------------- ENTRIES ----------------------

#[derive(Debug, Deserialize)]
pub struct LogEntryInput {
    pub log_date: String,
    pub fdc_id: Option<i64>,
    pub description: String,
    pub amount: f64,
    pub unit: String,
    pub grams: Option<f64>,
    pub calories_kcal: f64,
    pub protein_g: f64,
    pub fat_g: f64,
    pub carbs_g: f64,
    pub fiber_g: f64,
    pub fluid_oz: f64,
    pub meal: Option<String>,
    pub source: Option<String>, // "usda" | "custom" | "water"
}

pub fn log_intake(conn: &Connection, input: &LogEntryInput) -> Result<IntakeEntry> {
    // Validate date format
    if NaiveDate::parse_from_str(&input.log_date, "%Y-%m-%d").is_err() {
        return Err(AppError::InvalidDate(input.log_date.clone()));
    }

    let source = input.source.as_deref().unwrap_or("usda");

    conn.execute(
        "INSERT INTO intake_entries (
            log_date, fdc_id, description, amount, unit, grams,
            calories_kcal, protein_g, fat_g, carbs_g, fiber_g, fluid_oz,
            meal, source
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            &input.log_date,
            input.fdc_id,
            &input.description,
            input.amount,
            &input.unit,
            input.grams,
            input.calories_kcal,
            input.protein_g,
            input.fat_g,
            input.carbs_g,
            input.fiber_g,
            input.fluid_oz,
            &input.meal,
            source,
        ],
    )?;

    let id = conn.last_insert_rowid();

    get_entry_by_id(conn, id)
}

pub fn get_entry_by_id(conn: &Connection, id: i64) -> Result<IntakeEntry> {
    let entry = conn.query_row(
        "SELECT id, log_date, fdc_id, description, amount, unit, grams,
                calories_kcal, protein_g, fat_g, carbs_g, fiber_g, fluid_oz,
                meal, source, created_at
         FROM intake_entries WHERE id = ?1",
        [id],
        |row| {
            Ok(IntakeEntry {
                id: row.get(0)?,
                log_date: row.get(1)?,
                fdc_id: row.get(2)?,
                description: row.get(3)?,
                amount: row.get(4)?,
                unit: row.get(5)?,
                grams: row.get(6)?,
                calories_kcal: row.get(7)?,
                protein_g: row.get(8)?,
                fat_g: row.get(9)?,
                carbs_g: row.get(10)?,
                fiber_g: row.get(11)?,
                fluid_oz: row.get(12)?,
                meal: row.get(13)?,
                source: row.get(14)?,
                created_at: row.get(15)?,
            })
        },
    )?;
    Ok(entry)
}

pub fn get_entries_for_date(conn: &Connection, date: &str) -> Result<Vec<IntakeEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, log_date, fdc_id, description, amount, unit, grams,
                calories_kcal, protein_g, fat_g, carbs_g, fiber_g, fluid_oz,
                meal, source, created_at
         FROM intake_entries
         WHERE log_date = ?1
         ORDER BY created_at ASC, id ASC",
    )?;
    let entries = stmt
        .query_map([date], |row| {
            Ok(IntakeEntry {
                id: row.get(0)?,
                log_date: row.get(1)?,
                fdc_id: row.get(2)?,
                description: row.get(3)?,
                amount: row.get(4)?,
                unit: row.get(5)?,
                grams: row.get(6)?,
                calories_kcal: row.get(7)?,
                protein_g: row.get(8)?,
                fat_g: row.get(9)?,
                carbs_g: row.get(10)?,
                fiber_g: row.get(11)?,
                fluid_oz: row.get(12)?,
                meal: row.get(13)?,
                source: row.get(14)?,
                created_at: row.get(15)?,
            })
        })?
        .collect::<SqliteResult<Vec<_>>>()?;
    Ok(entries)
}

pub fn update_entry_amount(conn: &Connection, id: i64, new_amount: f64) -> Result<IntakeEntry> {
    // Re-fetch original to know grams + per-unit ratios
    let original = get_entry_by_id(conn, id)?;

    if new_amount <= 0.0 {
        return Err(AppError::InvalidInput("Amount must be positive".into()));
    }

    let ratio = if original.amount > 0.0 {
        new_amount / original.amount
    } else {
        1.0
    };

    // Scale all nutrients (water/fluid is independent of weight usually)
    let new_cals = original.calories_kcal * ratio;
    let new_pro = original.protein_g * ratio;
    let new_fat = original.fat_g * ratio;
    let new_carb = original.carbs_g * ratio;
    let new_fib = original.fiber_g * ratio;

    // For water entries we usually keep fluid_oz as the "amount", so scale it too if source=water
    let new_fluid = if original.source == "water" {
        original.fluid_oz * ratio
    } else {
        original.fluid_oz
    };

    conn.execute(
        "UPDATE intake_entries SET
            amount = ?1,
            calories_kcal = ?2,
            protein_g = ?3,
            fat_g = ?4,
            carbs_g = ?5,
            fiber_g = ?6,
            fluid_oz = ?7,
            updated_at = CURRENT_TIMESTAMP
         WHERE id = ?8",
        params![new_amount, new_cals, new_pro, new_fat, new_carb, new_fib, new_fluid, id],
    )?;

    get_entry_by_id(conn, id)
}

pub fn delete_entry(conn: &Connection, id: i64) -> Result<()> {
    let affected = conn.execute("DELETE FROM intake_entries WHERE id = ?1", [id])?;
    if affected == 0 {
        return Err(AppError::NotFound(format!("Entry {id} not found")));
    }
    Ok(())
}

// ---------------------- DAILY TOTALS & AVERAGES ----------------------

pub fn get_daily_totals(conn: &Connection, date: &str) -> Result<DailyTotals> {
    let mut stmt = conn.prepare(
        "SELECT 
            ?1 as log_date,
            COALESCE(SUM(calories_kcal), 0),
            COALESCE(SUM(protein_g), 0),
            COALESCE(SUM(fat_g), 0),
            COALESCE(SUM(carbs_g), 0),
            COALESCE(SUM(fiber_g), 0),
            COALESCE(SUM(fluid_oz), 0),
            COUNT(*)
         FROM intake_entries
         WHERE log_date = ?1",
    )?;

    let totals = stmt.query_row([date], |row| {
        Ok(DailyTotals {
            log_date: row.get(0)?,
            calories_kcal: row.get(1)?,
            protein_g: row.get(2)?,
            fat_g: row.get(3)?,
            carbs_g: row.get(4)?,
            fiber_g: row.get(5)?,
            fluid_oz: row.get(6)?,
            entry_count: row.get(7)?,
        })
    })?;
    Ok(totals)
}

pub fn get_averages(conn: &Connection, days: i32) -> Result<Averages> {
    if days <= 0 {
        return Ok(Averages { days, ..Default::default() });
    }

    // Use a subquery to get daily sums for the last N calendar days that have any data
    let sql = format!(
        "SELECT 
            AVG(daily_cals),
            AVG(daily_pro),
            AVG(daily_fat),
            AVG(daily_carb),
            AVG(daily_fib),
            AVG(daily_fluid)
         FROM (
            SELECT 
                log_date,
                SUM(calories_kcal) as daily_cals,
                SUM(protein_g)     as daily_pro,
                SUM(fat_g)         as daily_fat,
                SUM(carbs_g)       as daily_carb,
                SUM(fiber_g)       as daily_fib,
                SUM(fluid_oz)      as daily_fluid
            FROM intake_entries
            GROUP BY log_date
            ORDER BY log_date DESC
            LIMIT ?
         )"
    );

    let mut stmt = conn.prepare(&sql)?;
    let avg = stmt.query_row([days], |row| {
        Ok(Averages {
            days,
            calories_kcal: row.get::<_, Option<f64>>(0)?.unwrap_or(0.0),
            protein_g: row.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
            fat_g: row.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
            carbs_g: row.get::<_, Option<f64>>(3)?.unwrap_or(0.0),
            fiber_g: row.get::<_, Option<f64>>(4)?.unwrap_or(0.0),
            fluid_oz: row.get::<_, Option<f64>>(5)?.unwrap_or(0.0),
        })
    })?;

    Ok(avg)
}

// ---------------------- HISTORY ----------------------

pub fn get_history_summary(conn: &Connection, limit: i32) -> Result<Vec<HistoryDay>> {
    let limit = if limit <= 0 { 30 } else { limit.min(365) };

    let sql = "
        SELECT 
            log_date,
            COALESCE(SUM(calories_kcal), 0),
            COALESCE(SUM(protein_g), 0),
            COALESCE(SUM(fat_g), 0),
            COALESCE(SUM(carbs_g), 0),
            COALESCE(SUM(fiber_g), 0),
            COALESCE(SUM(fluid_oz), 0),
            COUNT(*)
        FROM intake_entries
        GROUP BY log_date
        ORDER BY log_date DESC
        LIMIT ?
    ";

    let mut stmt = conn.prepare(sql)?;
    let days = stmt
        .query_map([limit], |row| {
            Ok(HistoryDay {
                log_date: row.get(0)?,
                calories_kcal: row.get(1)?,
                protein_g: row.get(2)?,
                fat_g: row.get(3)?,
                carbs_g: row.get(4)?,
                fiber_g: row.get(5)?,
                fluid_oz: row.get(6)?,
                entry_count: row.get(7)?,
            })
        })?
        .collect::<SqliteResult<Vec<_>>>()?;
    Ok(days)
}

// ---------------------- FOOD CACHE (Phase 1 stub — full in Phase 2) ----------------------

pub fn get_cached_food(conn: &Connection, fdc_id: i64) -> Result<Option<serde_json::Value>> {
    let val: Option<String> = conn
        .query_row(
            "SELECT nutrients_json FROM food_cache WHERE fdc_id = ?1",
            [fdc_id],
            |r| r.get(0),
        )
        .optional()?;

    match val {
        Some(json_str) => Ok(Some(serde_json::from_str(&json_str)?)),
        None => Ok(None),
    }
}

// ---------------------- CUSTOM FOODS ----------------------

pub fn get_custom_foods(conn: &Connection) -> Result<Vec<CustomFood>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, kcal_per_100g, protein_per_100g, fat_per_100g, carbs_per_100g, fiber_per_100g,
                basis_unit, density_g_per_ml
         FROM custom_foods
         ORDER BY name COLLATE NOCASE ASC"
    )?;

    let foods = stmt
        .query_map([], |row| {
            Ok(CustomFood {
                id: row.get(0)?,
                name: row.get(1)?,
                kcal_per_100g: row.get(2)?,
                protein_per_100g: row.get(3)?,
                fat_per_100g: row.get(4)?,
                carbs_per_100g: row.get(5)?,
                fiber_per_100g: row.get(6)?,
                basis_unit: row.get(7)?,
                density_g_per_ml: row.get(8)?,
            })
        })?
        .collect::<SqliteResult<Vec<_>>>()?;
    Ok(foods)
}

pub fn save_custom_food(conn: &Connection, food: &CustomFood) -> Result<i64> {
    if food.id == 0 {
        // Insert new
        conn.execute(
            "INSERT INTO custom_foods (
                name, kcal_per_100g, protein_per_100g, fat_per_100g, carbs_per_100g, fiber_per_100g,
                basis_unit, density_g_per_ml
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                food.name,
                food.kcal_per_100g,
                food.protein_per_100g,
                food.fat_per_100g,
                food.carbs_per_100g,
                food.fiber_per_100g,
                food.basis_unit,
                food.density_g_per_ml,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    } else {
        // Update existing
        conn.execute(
            "UPDATE custom_foods SET
                name = ?1,
                kcal_per_100g = ?2,
                protein_per_100g = ?3,
                fat_per_100g = ?4,
                carbs_per_100g = ?5,
                fiber_per_100g = ?6,
                basis_unit = ?7,
                density_g_per_ml = ?8,
                updated_at = CURRENT_TIMESTAMP
             WHERE id = ?9",
            params![
                food.name,
                food.kcal_per_100g,
                food.protein_per_100g,
                food.fat_per_100g,
                food.carbs_per_100g,
                food.fiber_per_100g,
                food.basis_unit,
                food.density_g_per_ml,
                food.id,
            ],
        )?;
        Ok(food.id)
    }
}

pub fn save_custom_food_from_label(conn: &Connection, label: &CustomFoodLabel) -> Result<i64> {
    let name = label.name.trim();
    if name.is_empty() {
        return Err(AppError::InvalidInput("Food name is required.".into()));
    }
    let unit = crate::units::parse_serving_unit(&label.unit)?;
    let grams =
        crate::units::grams_for_serving(label.amount, &label.unit, label.density_g_per_ml)?;
    let per_100g = crate::units::per_100g_from_serving(
        crate::units::MacroTotals {
            kcal: label.kcal,
            protein: label.protein,
            fat: label.fat,
            carbs: label.carbs,
            fiber: label.fiber,
        },
        grams,
    )?;
    save_custom_food(
        conn,
        &CustomFood {
            id: 0,
            name: name.to_string(),
            kcal_per_100g: per_100g.kcal,
            protein_per_100g: per_100g.protein,
            fat_per_100g: per_100g.fat,
            carbs_per_100g: per_100g.carbs,
            fiber_per_100g: per_100g.fiber,
            basis_unit: unit.canonical().to_string(),
            density_g_per_ml: if unit.is_volume() {
                label.density_g_per_ml
            } else {
                None
            },
        },
    )
}

pub fn delete_custom_food(conn: &Connection, id: i64) -> Result<()> {
    let affected = conn.execute("DELETE FROM custom_foods WHERE id = ?1", [id])?;
    if affected == 0 {
        return Err(AppError::NotFound(format!("Custom food {id} not found")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "nomnom-nanny-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn migration_keeps_existing_foods_and_intake() {
        let dir = temp_dir("migrate");
        let db_path = dir.join("nomnom_nanny.db");
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                r#"
                CREATE TABLE schema_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                INSERT INTO schema_migrations (version) VALUES (2);
                CREATE TABLE custom_foods (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL,
                    kcal_per_100g REAL NOT NULL DEFAULT 0,
                    protein_per_100g REAL NOT NULL DEFAULT 0,
                    fat_per_100g REAL NOT NULL DEFAULT 0,
                    carbs_per_100g REAL NOT NULL DEFAULT 0,
                    fiber_per_100g REAL NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                INSERT INTO custom_foods (name, kcal_per_100g, protein_per_100g, fat_per_100g, carbs_per_100g, fiber_per_100g)
                VALUES ('yogurt', 97, 10, 2, 4, 0);
                CREATE TABLE intake_entries (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    log_date TEXT NOT NULL,
                    fdc_id INTEGER,
                    description TEXT NOT NULL,
                    amount REAL NOT NULL,
                    unit TEXT NOT NULL,
                    grams REAL,
                    calories_kcal REAL NOT NULL DEFAULT 0,
                    protein_g REAL NOT NULL DEFAULT 0,
                    fat_g REAL NOT NULL DEFAULT 0,
                    carbs_g REAL NOT NULL DEFAULT 0,
                    fiber_g REAL NOT NULL DEFAULT 0,
                    fluid_oz REAL NOT NULL DEFAULT 0,
                    meal TEXT,
                    source TEXT NOT NULL DEFAULT 'usda',
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                INSERT INTO intake_entries (
                    log_date, description, amount, unit, grams,
                    calories_kcal, protein_g, fat_g, carbs_g, fiber_g, fluid_oz, source
                ) VALUES (
                    '2026-10-01', 'yogurt', 150, 'g', 150,
                    145.5, 15, 3, 6, 0, 0, 'custom'
                );
                CREATE TABLE goals (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    calories_kcal REAL NOT NULL DEFAULT 2000,
                    protein_g REAL NOT NULL DEFAULT 150,
                    fat_g REAL NOT NULL DEFAULT 65,
                    carbs_g REAL NOT NULL DEFAULT 225,
                    fiber_g REAL NOT NULL DEFAULT 30,
                    hydration_oz REAL NOT NULL DEFAULT 64,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                INSERT INTO goals (id) VALUES (1);
                "#,
            )
            .unwrap();
        }

        let state = init_db(dir.clone()).unwrap();
        let conn = state.lock().unwrap();
        let foods = get_custom_foods(&conn).unwrap();
        assert_eq!(foods.len(), 1);
        assert_eq!(foods[0].name, "yogurt");
        assert_eq!(foods[0].kcal_per_100g, 97.0);
        assert_eq!(foods[0].basis_unit, "g");
        assert!(foods[0].density_g_per_ml.is_none());

        let entries = get_entries_for_date(&conn, "2026-10-01").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].description, "yogurt");
        assert_eq!(entries[0].amount, 150.0);
        assert_eq!(entries[0].unit, "g");
        assert_eq!(entries[0].calories_kcal, 145.5);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn history_day_returns_foods_with_amounts_and_macros() {
        let dir = temp_dir("history");
        let state = init_db(dir.clone()).unwrap();
        let conn = state.lock().unwrap();

        log_intake(
            &conn,
            &LogEntryInput {
                log_date: "2026-10-03".into(),
                fdc_id: None,
                description: "oats".into(),
                amount: 40.0,
                unit: "g".into(),
                grams: Some(40.0),
                calories_kcal: 150.0,
                protein_g: 5.0,
                fat_g: 3.0,
                carbs_g: 27.0,
                fiber_g: 4.0,
                fluid_oz: 0.0,
                meal: None,
                source: Some("custom".into()),
            },
        )
        .unwrap();
        log_intake(
            &conn,
            &LogEntryInput {
                log_date: "2026-10-03".into(),
                fdc_id: None,
                description: "milk".into(),
                amount: 240.0,
                unit: "ml".into(),
                grams: Some(247.2),
                calories_kcal: 120.0,
                protein_g: 8.0,
                fat_g: 5.0,
                carbs_g: 12.0,
                fiber_g: 0.0,
                fluid_oz: 0.0,
                meal: None,
                source: Some("custom".into()),
            },
        )
        .unwrap();
        log_intake(
            &conn,
            &LogEntryInput {
                log_date: "2026-10-04".into(),
                fdc_id: None,
                description: "apple".into(),
                amount: 1.0,
                unit: "g".into(),
                grams: Some(180.0),
                calories_kcal: 95.0,
                protein_g: 0.5,
                fat_g: 0.3,
                carbs_g: 25.0,
                fiber_g: 4.4,
                fluid_oz: 0.0,
                meal: None,
                source: Some("usda".into()),
            },
        )
        .unwrap();

        let summary = get_history_summary(&conn, 14).unwrap();
        let october_3 = summary.iter().find(|day| day.log_date == "2026-10-03").unwrap();
        assert_eq!(october_3.calories_kcal, 270.0);
        assert_eq!(october_3.entry_count, 2);

        let foods = get_entries_for_date(&conn, "2026-10-03").unwrap();
        assert_eq!(foods.len(), 2);
        assert_eq!(foods[0].description, "oats");
        assert_eq!(foods[0].amount, 40.0);
        assert_eq!(foods[0].unit, "g");
        assert_eq!(foods[0].protein_g, 5.0);
        assert_eq!(foods[0].fat_g, 3.0);
        assert_eq!(foods[0].carbs_g, 27.0);
        assert_eq!(foods[0].fiber_g, 4.0);
        assert_eq!(foods[1].description, "milk");
        assert_eq!(foods[1].amount, 240.0);
        assert_eq!(foods[1].unit, "ml");
        assert_eq!(foods[1].calories_kcal, 120.0);
        assert_eq!(foods[1].carbs_g, 12.0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn label_in_milliliters_survives_as_per_100g() {
        let dir = temp_dir("label");
        let state = init_db(dir.clone()).unwrap();
        let conn = state.lock().unwrap();
        save_custom_food_from_label(
            &conn,
            &CustomFoodLabel {
                name: "olive oil".into(),
                amount: 15.0,
                unit: "ml".into(),
                density_g_per_ml: Some(0.91),
                kcal: 120.0,
                protein: 0.0,
                fat: 14.0,
                carbs: 0.0,
                fiber: 0.0,
            },
        )
        .unwrap();
        let food = get_custom_foods(&conn).unwrap().pop().unwrap();
        assert_eq!(food.basis_unit, "ml");
        assert_eq!(food.density_g_per_ml, Some(0.91));
        let grams = crate::units::grams_for_serving(15.0, "ml", food.density_g_per_ml).unwrap();
        let scaled = crate::units::macros_for_grams(
            crate::units::MacroTotals {
                kcal: food.kcal_per_100g,
                protein: food.protein_per_100g,
                fat: food.fat_per_100g,
                carbs: food.carbs_per_100g,
                fiber: food.fiber_per_100g,
            },
            grams,
        )
        .unwrap();
        assert!((scaled.kcal - 120.0).abs() < 1e-6);
        assert!((scaled.fat - 14.0).abs() < 1e-6);
        let _ = fs::remove_dir_all(&dir);
    }
}
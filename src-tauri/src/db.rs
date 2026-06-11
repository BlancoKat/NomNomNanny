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
        "SELECT id, name, kcal_per_100g, protein_per_100g, fat_per_100g, carbs_per_100g, fiber_per_100g
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
            })
        })?
        .collect::<SqliteResult<Vec<_>>>()?;
    Ok(foods)
}

pub fn save_custom_food(conn: &Connection, food: &CustomFood) -> Result<i64> {
    if food.id == 0 {
        // Insert new
        conn.execute(
            "INSERT INTO custom_foods (name, kcal_per_100g, protein_per_100g, fat_per_100g, carbs_per_100g, fiber_per_100g)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                food.name,
                food.kcal_per_100g,
                food.protein_per_100g,
                food.fat_per_100g,
                food.carbs_per_100g,
                food.fiber_per_100g,
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
                updated_at = CURRENT_TIMESTAMP
             WHERE id = ?7",
            params![
                food.name,
                food.kcal_per_100g,
                food.protein_per_100g,
                food.fat_per_100g,
                food.carbs_per_100g,
                food.fiber_per_100g,
                food.id,
            ],
        )?;
        Ok(food.id)
    }
}

pub fn delete_custom_food(conn: &Connection, id: i64) -> Result<()> {
    let affected = conn.execute("DELETE FROM custom_foods WHERE id = ?1", [id])?;
    if affected == 0 {
        return Err(AppError::NotFound(format!("Custom food {id} not found")));
    }
    Ok(())
}
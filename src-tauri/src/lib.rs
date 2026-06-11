mod db;
mod error;
mod usda;

use db::{
    get_averages, get_daily_totals, get_entries_for_date, get_goals, get_history_summary,
    init_db, log_intake, update_entry_amount, update_goals, LogEntryInput,
};
use error::AppError;
use tauri::{Manager, State};

use crate::db::{delete_entry, DbState};

// Keep a simple greet for smoke tests
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

// ---------------- GOALS ----------------

#[tauri::command]
async fn get_goals_cmd(state: State<'_, DbState>) -> Result<db::Goal, AppError> {
    let conn = state.lock().map_err(|e| AppError::Internal(e.to_string()))?;
    get_goals(&conn)
}

#[tauri::command]
async fn update_goals_cmd(state: State<'_, DbState>, goal: db::Goal) -> Result<(), AppError> {
    let conn = state.lock().map_err(|e| AppError::Internal(e.to_string()))?;
    update_goals(&conn, &goal)
}

// ---------------- ENTRIES ----------------

#[tauri::command]
async fn log_intake_cmd(
    state: State<'_, DbState>,
    input: LogEntryInput,
) -> Result<db::IntakeEntry, AppError> {
    let conn = state.lock().map_err(|e| AppError::Internal(e.to_string()))?;
    log_intake(&conn, &input)
}

#[tauri::command]
async fn get_entries_for_date_cmd(
    state: State<'_, DbState>,
    date: String,
) -> Result<Vec<db::IntakeEntry>, AppError> {
    let conn = state.lock().map_err(|e| AppError::Internal(e.to_string()))?;
    get_entries_for_date(&conn, &date)
}

#[tauri::command]
async fn update_entry_amount_cmd(
    state: State<'_, DbState>,
    id: i64,
    new_amount: f64,
) -> Result<db::IntakeEntry, AppError> {
    let conn = state.lock().map_err(|e| AppError::Internal(e.to_string()))?;
    update_entry_amount(&conn, id, new_amount)
}

#[tauri::command]
async fn delete_entry_cmd(state: State<'_, DbState>, id: i64) -> Result<(), AppError> {
    let conn = state.lock().map_err(|e| AppError::Internal(e.to_string()))?;
    delete_entry(&conn, id)
}

// ---------------- DAILY + STATS ----------------

#[tauri::command]
async fn get_daily_totals_cmd(
    state: State<'_, DbState>,
    date: String,
) -> Result<db::DailyTotals, AppError> {
    let conn = state.lock().map_err(|e| AppError::Internal(e.to_string()))?;
    get_daily_totals(&conn, &date)
}

#[tauri::command]
async fn get_averages_cmd(
    state: State<'_, DbState>,
    days: i32,
) -> Result<db::Averages, AppError> {
    let conn = state.lock().map_err(|e| AppError::Internal(e.to_string()))?;
    get_averages(&conn, days)
}

#[tauri::command]
async fn get_history_summary_cmd(
    state: State<'_, DbState>,
    limit: i32,
) -> Result<Vec<db::HistoryDay>, AppError> {
    let conn = state.lock().map_err(|e| AppError::Internal(e.to_string()))?;
    get_history_summary(&conn, limit)
}

// ---------------- FOOD CACHE (stubs for Phase 2) ----------------

#[tauri::command]
async fn get_cached_food_cmd(
    state: State<'_, DbState>,
    fdc_id: i64,
) -> Result<Option<serde_json::Value>, AppError> {
    let conn = state.lock().map_err(|e| AppError::Internal(e.to_string()))?;
    db::get_cached_food(&conn, fdc_id)
}

// ---------------------- CUSTOM FOODS ----------------------

#[tauri::command]
async fn get_custom_foods_cmd(state: State<'_, DbState>) -> Result<Vec<db::CustomFood>, AppError> {
    let conn = state.lock().map_err(|e| AppError::Internal(e.to_string()))?;
    db::get_custom_foods(&conn)
}

#[tauri::command]
async fn save_custom_food_cmd(
    state: State<'_, DbState>,
    food: db::CustomFood,
) -> Result<i64, AppError> {
    let conn = state.lock().map_err(|e| AppError::Internal(e.to_string()))?;
    db::save_custom_food(&conn, &food)
}

#[tauri::command]
async fn delete_custom_food_cmd(state: State<'_, DbState>, id: i64) -> Result<(), AppError> {
    let conn = state.lock().map_err(|e| AppError::Internal(e.to_string()))?;
    db::delete_custom_food(&conn, id)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Initialize SQLite DB in app data directory
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to get app data dir");

            let db = init_db(app_data_dir)?;
            app.manage(db);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            // Goals
            get_goals_cmd,
            update_goals_cmd,
            // Intake
            log_intake_cmd,
            get_entries_for_date_cmd,
            update_entry_amount_cmd,
            delete_entry_cmd,
            // Stats
            get_daily_totals_cmd,
            get_averages_cmd,
            get_history_summary_cmd,
            // Cache (Phase 2 will expand)
            get_cached_food_cmd,
            // USDA FDC
            usda::search_usda_foods_cmd,
            usda::fetch_usda_food_details_cmd,
            usda::scale_nutrients_cmd,
            // Custom Foods
            get_custom_foods_cmd,
            save_custom_food_cmd,
            delete_custom_food_cmd
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

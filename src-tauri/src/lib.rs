#[cfg(target_os = "android")]
mod android_file;
#[cfg(target_os = "linux")]
mod appimage_runtime;
mod db;
mod diary;
mod error;
mod usda;
mod user_file;

use db::{
    get_averages, get_daily_totals, get_entries_for_date, get_goals, get_history_summary, init_db,
    log_intake, update_entry_amount, update_goals, LogEntryInput,
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
    let conn = state
        .lock()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    get_goals(&conn)
}

#[tauri::command]
async fn update_goals_cmd(state: State<'_, DbState>, goal: db::Goal) -> Result<(), AppError> {
    let conn = state
        .lock()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    update_goals(&conn, &goal)
}

// ---------------- ENTRIES ----------------

#[tauri::command]
async fn log_intake_cmd(
    state: State<'_, DbState>,
    input: LogEntryInput,
) -> Result<db::IntakeEntry, AppError> {
    let conn = state
        .lock()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    log_intake(&conn, &input)
}

#[tauri::command]
async fn get_entries_for_date_cmd(
    state: State<'_, DbState>,
    date: String,
) -> Result<Vec<db::IntakeEntry>, AppError> {
    let conn = state
        .lock()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    get_entries_for_date(&conn, &date)
}

#[tauri::command]
async fn update_entry_amount_cmd(
    state: State<'_, DbState>,
    id: i64,
    new_amount: f64,
) -> Result<db::IntakeEntry, AppError> {
    let conn = state
        .lock()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    update_entry_amount(&conn, id, new_amount)
}

#[tauri::command]
async fn delete_entry_cmd(state: State<'_, DbState>, id: i64) -> Result<(), AppError> {
    let conn = state
        .lock()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    delete_entry(&conn, id)
}

// ---------------- DAILY + STATS ----------------

#[tauri::command]
async fn get_daily_totals_cmd(
    state: State<'_, DbState>,
    date: String,
) -> Result<db::DailyTotals, AppError> {
    let conn = state
        .lock()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    get_daily_totals(&conn, &date)
}

#[tauri::command]
async fn get_averages_cmd(state: State<'_, DbState>, days: i32) -> Result<db::Averages, AppError> {
    let conn = state
        .lock()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    get_averages(&conn, days)
}

#[tauri::command]
async fn get_history_summary_cmd(
    state: State<'_, DbState>,
    limit: i32,
) -> Result<Vec<db::HistoryDay>, AppError> {
    let conn = state
        .lock()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    get_history_summary(&conn, limit)
}

// ---------------- FOOD CACHE (stubs for Phase 2) ----------------

#[tauri::command]
async fn get_cached_food_cmd(
    state: State<'_, DbState>,
    fdc_id: i64,
) -> Result<Option<serde_json::Value>, AppError> {
    let conn = state
        .lock()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    db::get_cached_food(&conn, fdc_id)
}

// ---------------------- CUSTOM FOODS ----------------------

#[tauri::command]
async fn get_custom_foods_cmd(state: State<'_, DbState>) -> Result<Vec<db::CustomFood>, AppError> {
    let conn = state
        .lock()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    db::get_custom_foods(&conn)
}

#[tauri::command]
async fn save_custom_food_cmd(
    state: State<'_, DbState>,
    food: db::CustomFood,
) -> Result<i64, AppError> {
    let conn = state
        .lock()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    db::save_custom_food(&conn, &food)
}

#[tauri::command]
async fn delete_custom_food_cmd(state: State<'_, DbState>, id: i64) -> Result<(), AppError> {
    let conn = state
        .lock()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    db::delete_custom_food(&conn, id)
}

// ---------------------- DIARY FILE ----------------------

#[tauri::command]
fn export_diary_cmd(state: State<'_, DbState>) -> Result<String, AppError> {
    let conn = state
        .lock()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    diary::export_diary(&conn)
}

#[tauri::command]
fn preview_diary_import_cmd(
    state: State<'_, DbState>,
    contents: String,
) -> Result<diary::DiaryImportPreview, AppError> {
    let conn = state
        .lock()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    diary::preview_diary(&conn, &contents)
}

/// Replaces goals, intake entries, and custom foods. `confirm` must be true;
/// the UI sets it only after the user reviews the preview.
#[tauri::command]
fn import_diary_cmd(
    state: State<'_, DbState>,
    contents: String,
    confirm: bool,
) -> Result<(), AppError> {
    if !confirm {
        return Err(AppError::InvalidInput(
            "Import was not confirmed, so the diary on this device was left as it is.".into(),
        ));
    }
    let conn = state
        .lock()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    diary::import_diary(&conn, &contents)
}

#[tauri::command]
fn write_diary_file_cmd(
    app: tauri::AppHandle,
    path: String,
    contents: String,
) -> Result<(), AppError> {
    user_file::write_user_file(&app, &path, &contents)
}

#[tauri::command]
fn read_diary_file_cmd(app: tauri::AppHandle, path: String) -> Result<String, AppError> {
    user_file::read_user_file(&app, &path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    appimage_runtime::prefer_host_wayland_client();

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
            delete_custom_food_cmd,
            // Diary file
            export_diary_cmd,
            preview_diary_import_cmd,
            import_diary_cmd,
            write_diary_file_cmd,
            read_diary_file_cmd
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

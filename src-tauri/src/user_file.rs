//! Save and open the diary through a path the system file dialog returned.
//!
//! Desktop dialogs return a filesystem path. Android dialogs return a
//! `content://` URI from the create-document and open-document pickers.

use tauri::AppHandle;

use crate::error::{AppError, Result};

pub fn write_user_file(app: &AppHandle, path: &str, contents: &str) -> Result<()> {
    if path.trim().is_empty() {
        return Err(AppError::InvalidInput("No file was selected.".into()));
    }
    #[cfg(target_os = "android")]
    if is_android_uri(path) {
        return crate::android_file::write_uri(app, path, contents);
    }
    #[cfg(not(target_os = "android"))]
    let _ = app;

    std::fs::write(path, contents)?;
    Ok(())
}

pub fn read_user_file(app: &AppHandle, path: &str) -> Result<String> {
    if path.trim().is_empty() {
        return Err(AppError::InvalidInput("No file was selected.".into()));
    }
    let bytes = {
        #[cfg(target_os = "android")]
        {
            if is_android_uri(path) {
                crate::android_file::read_uri(app, path)?
            } else {
                std::fs::read(path)?
            }
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = app;
            std::fs::read(path)?
        }
    };
    String::from_utf8(bytes).map_err(|_| {
        AppError::InvalidInput(
            "That file is not a text diary. Export a NomNom diary and choose that file.".into(),
        )
    })
}

#[cfg(target_os = "android")]
fn is_android_uri(path: &str) -> bool {
    path.contains("://")
}

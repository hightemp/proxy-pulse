#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod startup;

use proxy_pulse_core::{
    export::{self, ExportOptions, Payload},
    model::{AppError, AppResult, CheckSettings},
    parser::{ImportOptions, MAX_BYTES},
    session::{self, Preview, SharedSession, Snapshot},
};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    sync::{Arc, Mutex},
};
use tauri::{Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
fn snapshot(state: State<'_, SharedSession>, since: u64) -> AppResult<Snapshot> {
    Ok(session::lock(&state)?.snapshot(since))
}

#[tauri::command]
async fn preview_import(
    state: State<'_, SharedSession>,
    text: String,
    options: ImportOptions,
) -> AppResult<Preview> {
    let state = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || session::lock(&state)?.preview(&text, &options))
        .await
        .map_err(|_| AppError::new("INTERNAL_ERROR", "Import worker failed."))?
}

#[tauri::command]
fn commit_import(
    state: State<'_, SharedSession>,
    replace: bool,
    keep_duplicates: bool,
    include_invalid: bool,
) -> AppResult<usize> {
    session::lock(&state)?.commit_import(replace, keep_duplicates, include_invalid)
}

#[tauri::command]
fn start_check(
    state: State<'_, SharedSession>,
    ids: Vec<u64>,
    settings: CheckSettings,
    detect_again: bool,
) -> AppResult<u64> {
    session::start(Arc::clone(&state), ids, settings, detect_again)
}

#[tauri::command]
fn stop_check(state: State<'_, SharedSession>) -> AppResult<()> {
    if let Some(control) = &session::lock(&state)?.control {
        control.cancel();
    }
    Ok(())
}

#[tauri::command]
fn clear_entries(state: State<'_, SharedSession>, ids: Vec<u64>) -> AppResult<()> {
    session::lock(&state)?.clear(&ids)
}

#[tauri::command]
fn edit_entry(state: State<'_, SharedSession>, id: u64, text: String) -> AppResult<()> {
    session::lock(&state)?.edit(id, &text)
}

#[tauri::command]
fn reveal_entry(state: State<'_, SharedSession>, id: u64) -> AppResult<String> {
    let state = session::lock(&state)?;
    let entry = state
        .entries
        .iter()
        .find(|e| e.id == id)
        .ok_or_else(|| AppError::new("NOT_FOUND", "Record no longer exists."))?;
    Ok(entry.parsed.raw.clone())
}

#[tauri::command]
fn read_clipboard(app: tauri::AppHandle) -> AppResult<String> {
    app.clipboard()
        .read_text()
        .map_err(|_| AppError::new("CLIPBOARD_ERROR", "Could not read text from the clipboard."))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportedFile {
    text: String,
    source_name: String,
}

#[tauri::command]
async fn import_file(app: tauri::AppHandle) -> AppResult<Option<ImportedFile>> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(file) = app
            .dialog()
            .file()
            .add_filter("Proxy lists", &["txt", "csv", "tsv"])
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        let path = file
            .into_path()
            .map_err(|_| AppError::new("FILE_READ_FAILED", "Select a local file."))?;
        let file = std::fs::File::open(&path)
            .map_err(|_| AppError::new("FILE_READ_FAILED", "Cannot read the selected file."))?;
        let mut bytes = Vec::new();
        file.take((MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| AppError::new("FILE_READ_FAILED", "Could not read the complete file."))?;
        if bytes.len() > MAX_BYTES {
            return Err(AppError::new(
                "IMPORT_TOO_LARGE",
                "Import must not exceed 20 MiB.",
            ));
        }
        let text = String::from_utf8(bytes)
            .map_err(|_| AppError::new("INVALID_ENCODING", "The file must be encoded as UTF-8."))?;
        Ok(Some(ImportedFile {
            text,
            source_name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Imported file".into()),
        }))
    })
    .await
    .map_err(|_| AppError::new("INTERNAL_ERROR", "File dialog worker failed."))?
}

#[tauri::command]
async fn export_data(
    app: tauri::AppHandle,
    state: State<'_, SharedSession>,
    options: ExportOptions,
    destination: String,
) -> AppResult<Option<usize>> {
    let state = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || {
        let Payload {
            text,
            count,
            extension,
        } = export::render(&session::lock(&state)?.entries, &options)?;
        match destination.as_str() {
            "clipboard" => app.clipboard().write_text(text).map_err(|_| {
                AppError::new("CLIPBOARD_ERROR", "Could not write to the clipboard.")
            })?,
            "file" => {
                let Some(file) = app
                    .dialog()
                    .file()
                    .set_file_name(format!(
                        "proxy-pulse-{}.{}",
                        options.scope.to_lowercase(),
                        extension
                    ))
                    .add_filter("Proxy export", &[&extension])
                    .blocking_save_file()
                else {
                    return Ok(None);
                };
                let path = file.into_path().map_err(|_| {
                    AppError::new("FILE_WRITE_FAILED", "Select a local output file.")
                })?;
                export::save_atomic(&path, &text)?;
            }
            _ => return Err(AppError::new("INVALID_EXPORT", "Choose clipboard or file.")),
        }
        Ok(Some(count))
    })
    .await
    .map_err(|_| AppError::new("INTERNAL_ERROR", "Export worker failed."))?
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
struct Preferences {
    theme: String,
    concurrency: usize,
    rate_limit: u32,
}

#[tauri::command]
fn load_preferences(app: tauri::AppHandle) -> Preferences {
    app.path()
        .app_config_dir()
        .ok()
        .and_then(|path| std::fs::read(path.join("preferences.json")).ok())
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_default()
}

#[tauri::command]
fn save_preferences(app: tauri::AppHandle, preferences: Preferences) -> AppResult<()> {
    if !matches!(preferences.theme.as_str(), "system" | "light" | "dark")
        || !(1..=200).contains(&preferences.concurrency)
        || !(1..=100).contains(&preferences.rate_limit)
    {
        return Err(AppError::new(
            "INVALID_SETTINGS",
            "Invalid application preferences.",
        ));
    }
    let directory = app
        .path()
        .app_config_dir()
        .map_err(|_| AppError::new("SETTINGS_ERROR", "Cannot locate the settings folder."))?;
    std::fs::create_dir_all(&directory)
        .map_err(|_| AppError::new("SETTINGS_ERROR", "Cannot create the settings folder."))?;
    let text = serde_json::to_string_pretty(&preferences)
        .map_err(|_| AppError::new("SETTINGS_ERROR", "Cannot serialize settings."))?;
    export::save_atomic(&directory.join("preferences.json"), &text)
}

fn main() {
    // Environment changes must precede every runtime, plugin and worker thread.
    startup::configure_before_runtime();
    let state: SharedSession = Arc::new(Mutex::new(session::Session::default()));
    let result = tauri::Builder::default()
        .manage(state)
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            snapshot,
            preview_import,
            commit_import,
            start_check,
            stop_check,
            clear_entries,
            edit_entry,
            reveal_entry,
            read_clipboard,
            import_file,
            export_data,
            load_preferences,
            save_preferences
        ])
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                if let Ok(state) = session::lock(&window.state::<SharedSession>()) {
                    if let Some(control) = &state.control {
                        control.cancel();
                    }
                }
            }
        })
        .run(tauri::generate_context!());
    if result.is_err() {
        eprintln!("Proxy Pulse could not start. Verify the desktop runtime dependencies.");
        std::process::exit(1);
    }
}

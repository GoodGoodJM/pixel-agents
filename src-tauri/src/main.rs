// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod discovery;
mod jsonl_watcher;
mod layout;
mod project;
mod state;
mod tab_focus;

use serde_json::json;
use tauri::{AppHandle, Emitter, State};

/// Global app state managed by Tauri
struct AppState {
    discovery: discovery::SharedDiscoveryState,
}

// -- Tauri Commands ----------------------------------------------------------

#[tauri::command]
fn focus_agent(state: State<AppState>, id: i32) -> Result<(), String> {
    let discovery = state.discovery.lock().unwrap();
    let tab_index = discovery.get_tab_index(id)?;
    drop(discovery);
    tab_focus::focus_warp_tab(tab_index)
}

#[tauri::command]
fn webview_ready(state: State<AppState>, app: AppHandle) -> Result<(), String> {
    // Load and send settings
    let settings = state::load_settings();
    app.emit(
        "settingsLoaded",
        json!({
            "type": "settingsLoaded",
            "soundEnabled": settings.sound_enabled,
        }),
    )
    .map_err(|e| e.to_string())?;

    // Load and send layout
    if let Some(layout_val) = layout::read_layout() {
        app.emit(
            "layoutLoaded",
            json!({
                "type": "layoutLoaded",
                "layout": layout_val,
            }),
        )
        .map_err(|e| e.to_string())?;
    } else {
        app.emit(
            "layoutLoaded",
            json!({
                "type": "layoutLoaded",
                "layout": null,
            }),
        )
        .map_err(|e| e.to_string())?;
    }

    // Send already-discovered agents as existingAgents with appearance data
    let appearances = state::load_agent_appearances();
    let discovery = state.discovery.lock().unwrap();
    let mut agents = Vec::new();
    for (&id, session_id) in &discovery.id_to_session {
        let appearance = appearances.get(session_id);
        agents.push(json!({
            "id": id,
            "sessionId": session_id,
            "palette": appearance.and_then(|a| a.palette),
            "hueShift": appearance.and_then(|a| a.hue_shift),
            "seatId": appearance.and_then(|a| a.seat_id.as_deref()),
        }));
    }
    drop(discovery);

    if !agents.is_empty() {
        app.emit(
            "existingAgents",
            json!({
                "type": "existingAgents",
                "agents": agents,
            }),
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
fn save_layout(layout: serde_json::Value) -> Result<(), String> {
    layout::write_layout(&layout)
}

#[tauri::command]
fn save_agent_seats(seats: serde_json::Value) -> Result<(), String> {
    let mut appearances = state::load_agent_appearances();

    if let Some(obj) = seats.as_object() {
        for (session_id, v) in obj {
            let entry = appearances
                .entry(session_id.clone())
                .or_default();
            if let Some(seat_id) = v.get("seatId").and_then(|s| s.as_str()) {
                entry.seat_id = Some(seat_id.to_string());
            }
            if let Some(palette) = v.get("palette").and_then(|p| p.as_u64()) {
                entry.palette = Some(palette as u8);
            }
            if let Some(hue_shift) = v.get("hueShift").and_then(|h| h.as_f64()) {
                entry.hue_shift = Some(hue_shift);
            }
        }
    }

    state::save_agent_appearances(&appearances)
}

#[tauri::command]
fn set_sound_enabled(enabled: bool) -> Result<(), String> {
    let mut settings = state::load_settings();
    settings.sound_enabled = enabled;
    state::save_settings(&settings)
}

#[tauri::command]
async fn export_layout(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_dialog::DialogExt;

    let layout = layout::read_layout().ok_or("No layout to export")?;
    let json = serde_json::to_string_pretty(&layout).map_err(|e| e.to_string())?;

    let file = app
        .dialog()
        .file()
        .add_filter("JSON", &["json"])
        .set_file_name("pixel-agents-layout.json")
        .blocking_save_file();

    if let Some(path) = file {
        std::fs::write(path.as_path().unwrap(), json).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn import_layout(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_dialog::DialogExt;

    let file = app
        .dialog()
        .file()
        .add_filter("JSON", &["json"])
        .blocking_pick_file();

    if let Some(path) = file {
        let content =
            std::fs::read_to_string(path.as_path().unwrap()).map_err(|e| e.to_string())?;
        let imported: serde_json::Value =
            serde_json::from_str(&content).map_err(|_| "Invalid JSON".to_string())?;

        // Validate layout format
        if imported.get("version") != Some(&serde_json::json!(1))
            || !imported
                .get("tiles")
                .map_or(false, |t| t.is_array())
        {
            return Err("Invalid layout file: missing version or tiles".to_string());
        }

        layout::write_layout(&imported)?;
        app.emit(
            "layoutLoaded",
            json!({
                "type": "layoutLoaded",
                "layout": imported,
            }),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// -- Main --------------------------------------------------------------------

fn main() {
    let jsonl_watcher = jsonl_watcher::new_shared();
    let discovery = discovery::new_shared();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .manage(AppState {
            discovery: discovery.clone(),
        })
        .setup(move |app| {
            // Start JSONL polling loop
            jsonl_watcher::start_poll_loop(jsonl_watcher.clone(), app.handle().clone());
            // Start discovery loop (scans for Claude Code instances)
            discovery::start_discovery_loop(
                discovery.clone(),
                jsonl_watcher.clone(),
                app.handle().clone(),
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            focus_agent,
            webview_ready,
            save_layout,
            save_agent_seats,
            set_sound_enabled,
            export_layout,
            import_layout,
        ])
        .run(tauri::generate_context!())
        .expect("error while running pixel-agents");
}

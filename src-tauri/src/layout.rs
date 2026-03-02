use std::fs;
use std::path::PathBuf;

fn layout_dir() -> PathBuf {
    dirs::home_dir()
        .expect("could not determine home directory")
        .join(".pixel-agents")
}

fn layout_file_path() -> PathBuf {
    layout_dir().join("layout.json")
}

pub fn read_layout() -> Option<serde_json::Value> {
    let path = layout_file_path();
    if !path.exists() {
        return read_default_layout();
    }
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

fn read_default_layout() -> Option<serde_json::Value> {
    // Bundled default layout compiled into the binary
    const DEFAULT_LAYOUT_JSON: &str =
        include_str!("../../webview-ui/public/assets/default-layout.json");
    serde_json::from_str(DEFAULT_LAYOUT_JSON).ok()
}

pub fn write_layout(layout: &serde_json::Value) -> Result<(), String> {
    let path = layout_file_path();
    let dir = path.parent().unwrap();
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(layout).map_err(|e| e.to_string())?;
    // Atomic write via temp file + rename
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, &json).map_err(|e| e.to_string())?;
    fs::rename(&tmp_path, &path).map_err(|e| e.to_string())?;
    Ok(())
}

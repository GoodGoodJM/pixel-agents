use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Agent appearance data, keyed by session_id in agent-appearance.json
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentAppearance {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub palette: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hue_shift: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seat_id: Option<String>,
}

/// App-level settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default = "default_true")]
    pub sound_enabled: bool,
}

fn default_true() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            sound_enabled: true,
        }
    }
}

fn pixel_agents_dir() -> PathBuf {
    dirs::home_dir()
        .expect("could not determine home directory")
        .join(".pixel-agents")
}

fn settings_path() -> PathBuf {
    pixel_agents_dir().join("settings.json")
}

fn agent_appearance_path() -> PathBuf {
    pixel_agents_dir().join("agent-appearance.json")
}

pub fn load_settings() -> AppSettings {
    let path = settings_path();
    if !path.exists() {
        return AppSettings::default();
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = settings_path();
    let dir = path.parent().unwrap();
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())
}

/// Load agent appearances from ~/.pixel-agents/agent-appearance.json
pub fn load_agent_appearances() -> HashMap<String, AgentAppearance> {
    let path = agent_appearance_path();
    if !path.exists() {
        return HashMap::new();
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Save agent appearances to ~/.pixel-agents/agent-appearance.json
pub fn save_agent_appearances(appearances: &HashMap<String, AgentAppearance>) -> Result<(), String> {
    let path = agent_appearance_path();
    let dir = path.parent().unwrap();
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(appearances).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())
}

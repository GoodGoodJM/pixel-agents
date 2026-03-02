use serde_json::json;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::jsonl_watcher;
use crate::project;

const DISCOVERY_INTERVAL_MS: u64 = 2000;

/// Information about a discovered Claude Code instance
#[derive(Debug, Clone)]
struct DiscoveredAgent {
    /// Stable key: either `--session-id` value or `"pid:<pid>"` fallback
    stable_key: String,
    /// Actual JSONL session UUID (for watcher registration)
    session_id: String,
    cwd: String,
    shell_pid: u32,
}

/// Maps stable_key -> internal agent id
pub struct DiscoveryState {
    key_to_id: HashMap<String, i32>,
    id_to_key: HashMap<i32, String>,
    pub id_to_session: HashMap<i32, String>,
    /// shell PID for each agent, used for tab ordering
    id_to_shell_pid: HashMap<i32, u32>,
    next_agent_id: i32,
    /// Warp terminal-server PID (refreshed each scan)
    warp_server_pid: Option<u32>,
    /// ALL Warp shell PIDs (sorted), refreshed each scan
    all_warp_shell_pids: Vec<u32>,
}

pub type SharedDiscoveryState = Arc<Mutex<DiscoveryState>>;

impl DiscoveryState {
    fn new() -> Self {
        Self {
            key_to_id: HashMap::new(),
            id_to_key: HashMap::new(),
            id_to_session: HashMap::new(),
            id_to_shell_pid: HashMap::new(),
            next_agent_id: 1,
            warp_server_pid: None,
            all_warp_shell_pids: Vec::new(),
        }
    }

    /// Get tab index for an agent based on ALL Warp shell PIDs (not just Claude ones).
    /// This maps to the actual Warp tab position (Cmd+1, Cmd+2, ...).
    pub fn get_tab_index(&self, id: i32) -> Result<usize, String> {
        let target_shell_pid = self
            .id_to_shell_pid
            .get(&id)
            .ok_or_else(|| format!("Agent {} not found", id))?;

        // Find position in ALL Warp shell PIDs
        self.all_warp_shell_pids
            .iter()
            .position(|p| p == target_shell_pid)
            .ok_or_else(|| {
                format!(
                    "Shell PID {} not found in Warp tabs for agent {}",
                    target_shell_pid, id
                )
            })
    }
}

pub fn new_shared() -> SharedDiscoveryState {
    Arc::new(Mutex::new(DiscoveryState::new()))
}

/// Start the discovery loop that scans for Claude Code instances every 2 seconds.
pub fn start_discovery_loop(
    discovery: SharedDiscoveryState,
    jsonl_watcher: jsonl_watcher::SharedJsonlWatcher,
    app: AppHandle,
) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(DISCOVERY_INTERVAL_MS));

            // Refresh Warp terminal-server PID and all shell PIDs
            let warp_server_pid = find_warp_server_pid();
            let all_warp_shells = match warp_server_pid {
                Some(pid) => get_children_pids(pid),
                None => Vec::new(),
            };

            let discovered = scan_claude_instances();

            let mut disc = discovery.lock().unwrap();

            disc.warp_server_pid = warp_server_pid;
            disc.all_warp_shell_pids = all_warp_shells;

            // Build set of current stable keys
            let current_keys: std::collections::HashSet<String> =
                discovered.iter().map(|a| a.stable_key.clone()).collect();

            // Detect removed agents
            let existing_keys: Vec<String> = disc.key_to_id.keys().cloned().collect();
            for key in &existing_keys {
                if !current_keys.contains(key) {
                    if let Some(id) = disc.key_to_id.remove(key) {
                        disc.id_to_key.remove(&id);
                        disc.id_to_session.remove(&id);
                        disc.id_to_shell_pid.remove(&id);

                        // Unregister from JSONL watcher
                        jsonl_watcher::unregister_agent(&jsonl_watcher, id);

                        let _ = app.emit(
                            "agentClosed",
                            json!({
                                "type": "agentClosed",
                                "id": id,
                            }),
                        );
                    }
                }
            }

            // Detect new agents
            for agent in &discovered {
                if disc.key_to_id.contains_key(&agent.stable_key) {
                    // Update shell PID in case it changed (tab reordering)
                    if let Some(&id) = disc.key_to_id.get(&agent.stable_key) {
                        disc.id_to_shell_pid.insert(id, agent.shell_pid);
                    }
                    continue;
                }

                let id = disc.next_agent_id;
                disc.next_agent_id += 1;

                disc.key_to_id.insert(agent.stable_key.clone(), id);
                disc.id_to_key.insert(id, agent.stable_key.clone());
                disc.id_to_session.insert(id, agent.session_id.clone());
                disc.id_to_shell_pid.insert(id, agent.shell_pid);

                let folder_name = Path::new(&agent.cwd)
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| agent.cwd.clone());

                // Emit agentCreated BEFORE register_agent so the frontend
                // has the agent when replay_tail emits tool/status events
                let _ = app.emit(
                    "agentCreated",
                    json!({
                        "type": "agentCreated",
                        "id": id,
                        "sessionId": agent.session_id,
                        "folderName": folder_name,
                    }),
                );

                let project_dir = project::get_project_dir(&agent.cwd);
                jsonl_watcher::register_agent(
                    &jsonl_watcher,
                    id,
                    &agent.session_id,
                    &project_dir,
                    &app,
                );
            }

            drop(disc);
        }
    });
}

/// Scan system for running Claude Code instances
fn scan_claude_instances() -> Vec<DiscoveredAgent> {
    let pids = match find_claude_pids() {
        Some(pids) => pids,
        None => return Vec::new(),
    };

    let mut agents = Vec::new();

    for pid in pids {
        if let Some(agent) = inspect_claude_process(pid) {
            agents.push(agent);
        }
    }

    agents
}

/// Find all Claude Code PIDs via `ps`
fn find_claude_pids() -> Option<Vec<u32>> {
    let output = Command::new("ps")
        .args(["-eo", "pid,comm"])
        .output()
        .ok()?;

    if !output.status.success() {
        return Some(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let pids: Vec<u32> = stdout
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let (pid_str, comm) = trimmed.split_once(char::is_whitespace)?;
            if comm.trim() == "claude" {
                pid_str.trim().parse().ok()
            } else {
                None
            }
        })
        .collect();

    Some(pids)
}

/// Inspect a Claude process to extract session ID, CWD, and parent shell PID
fn inspect_claude_process(pid: u32) -> Option<DiscoveredAgent> {
    // Get parent PID and args
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "ppid=,args="])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let line = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if line.is_empty() {
        return None;
    }

    // Parse ppid and args
    let trimmed = line.trim();
    let (ppid_str, args) = trimmed.split_once(char::is_whitespace)?;
    let shell_pid: u32 = ppid_str.trim().parse().ok()?;

    // Get CWD via lsof
    let cwd = get_process_cwd(pid)?;

    // Extract --session-id from args
    let explicit_session = extract_session_id(args);

    let (stable_key, session_id) = if let Some(ref sid) = explicit_session {
        // Has explicit --session-id: use it as both stable key and session
        (sid.clone(), sid.clone())
    } else {
        // No --session-id: use PID as stable key, find session from JSONL
        let session = find_session_from_recent_jsonl(&cwd)
            .unwrap_or_else(|| format!("pid-{}", pid));
        (format!("pid:{}", pid), session)
    };

    Some(DiscoveredAgent {
        stable_key,
        session_id,
        cwd,
        shell_pid,
    })
}

/// Extract --session-id value from command args
fn extract_session_id(args: &str) -> Option<String> {
    let parts: Vec<&str> = args.split_whitespace().collect();
    for (i, part) in parts.iter().enumerate() {
        if *part == "--session-id" {
            if let Some(value) = parts.get(i + 1) {
                return Some(value.to_string());
            }
        }
        // Also handle --session-id=VALUE format
        if let Some(value) = part.strip_prefix("--session-id=") {
            return Some(value.to_string());
        }
    }
    None
}

/// Get the CWD of a process via lsof
fn get_process_cwd(pid: u32) -> Option<String> {
    let output = Command::new("lsof")
        .args(["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // lsof -Fn outputs lines starting with 'n' for the name (path)
    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix('n') {
            if path.starts_with('/') {
                return Some(path.to_string());
            }
        }
    }

    None
}

/// Find session ID from the most recently modified JSONL file in the project dir.
fn find_session_from_recent_jsonl(cwd: &str) -> Option<String> {
    let project_dir = project::get_project_dir(cwd);
    if !project_dir.exists() {
        return None;
    }

    // Find the most recently modified JSONL file
    let mut newest: Option<(PathBuf, std::time::SystemTime)> = None;
    if let Ok(entries) = std::fs::read_dir(&project_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                if let Ok(meta) = path.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if newest
                            .as_ref()
                            .map_or(true, |(_, prev_time)| modified > *prev_time)
                        {
                            newest = Some((path, modified));
                        }
                    }
                }
            }
        }
    }

    let (jsonl_path, _) = newest?;
    session_id_from_jsonl(&jsonl_path)
}

/// Extract sessionId from first line of a JSONL file, fallback to filename
fn session_id_from_jsonl(jsonl_path: &Path) -> Option<String> {
    let file = std::fs::File::open(jsonl_path).ok()?;
    let reader = BufReader::new(file);
    let first_line = reader.lines().next()?.ok()?;

    let record: serde_json::Value = serde_json::from_str(&first_line).ok()?;
    record["sessionId"]
        .as_str()
        .map(|s| s.to_string())
        .or_else(|| {
            jsonl_path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
        })
}

/// Find the Warp terminal-server PID using `ps` (pgrep is unreliable on macOS)
fn find_warp_server_pid() -> Option<u32> {
    let output = Command::new("ps")
        .args(["-eo", "pid,args"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.contains("terminal-server") && trimmed.contains("--parent-pid") {
            let pid_str = trimmed.split_whitespace().next()?;
            return pid_str.parse().ok();
        }
    }
    None
}

/// Get all direct children PIDs of a process (sorted ascending).
/// Uses `ps` instead of `pgrep -P` which can miss processes.
fn get_children_pids(parent_pid: u32) -> Vec<u32> {
    let output = match Command::new("ps")
        .args(["-eo", "pid,ppid"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    if !output.status.success() {
        return Vec::new();
    }

    let parent_str = parent_pid.to_string();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut pids: Vec<u32> = stdout
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let (pid_str, ppid_str) = trimmed.split_once(char::is_whitespace)?;
            if ppid_str.trim() == parent_str {
                pid_str.trim().parse().ok()
            } else {
                None
            }
        })
        .collect();
    pids.sort();
    pids
}

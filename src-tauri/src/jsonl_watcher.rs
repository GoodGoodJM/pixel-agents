use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const JSONL_POLL_INTERVAL_MS: u64 = 1000;
const TOOL_DONE_DELAY_MS: u64 = 300;
const PERMISSION_TIMER_DELAY_MS: u64 = 7000;
const TEXT_IDLE_DELAY_MS: u64 = 5000;
const BASH_COMMAND_DISPLAY_MAX_LENGTH: usize = 30;
const TASK_DESCRIPTION_DISPLAY_MAX_LENGTH: usize = 40;
const TAIL_READ_BYTES: u64 = 65536;

const PERMISSION_EXEMPT_TOOLS: &[&str] = &["Task", "Agent", "AskUserQuestion"];

fn is_exempt(tool_name: &str) -> bool {
    PERMISSION_EXEMPT_TOOLS.contains(&tool_name)
}

struct AgentWatchState {
    id: i32,
    jsonl_path: PathBuf,
    file_offset: u64,
    line_buffer: String,
    active_tool_ids: HashSet<String>,
    active_tool_names: HashMap<String, String>,
    active_tool_statuses: HashMap<String, String>,
    active_subagent_tool_ids: HashMap<String, HashSet<String>>,
    active_subagent_tool_names: HashMap<String, HashMap<String, String>>,
    is_waiting: bool,
    permission_sent: bool,
    had_tools_in_turn: bool,
}

impl AgentWatchState {
    fn new(id: i32, jsonl_path: PathBuf) -> Self {
        Self {
            id,
            jsonl_path,
            file_offset: 0,
            line_buffer: String::new(),
            active_tool_ids: HashSet::new(),
            active_tool_names: HashMap::new(),
            active_tool_statuses: HashMap::new(),
            active_subagent_tool_ids: HashMap::new(),
            active_subagent_tool_names: HashMap::new(),
            is_waiting: false,
            permission_sent: false,
            had_tools_in_turn: false,
        }
    }

    fn clear_activity(&mut self) {
        self.active_tool_ids.clear();
        self.active_tool_names.clear();
        self.active_tool_statuses.clear();
        self.active_subagent_tool_ids.clear();
        self.active_subagent_tool_names.clear();
        self.is_waiting = false;
        self.permission_sent = false;
    }

    fn has_non_exempt_tools(&self) -> bool {
        for tool_name in self.active_tool_names.values() {
            if !is_exempt(tool_name) {
                return true;
            }
        }
        for sub_names in self.active_subagent_tool_names.values() {
            for tool_name in sub_names.values() {
                if !is_exempt(tool_name) {
                    return true;
                }
            }
        }
        false
    }
}

fn format_tool_status(tool_name: &str, input: &Value) -> String {
    let basename = |p: &Value| -> String {
        p.as_str()
            .map(|s| {
                Path::new(s)
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_default()
            })
            .unwrap_or_default()
    };

    match tool_name {
        "Read" => format!("Reading {}", basename(&input["file_path"])),
        "Edit" => format!("Editing {}", basename(&input["file_path"])),
        "Write" => format!("Writing {}", basename(&input["file_path"])),
        "Bash" => {
            let cmd = input["command"].as_str().unwrap_or("");
            if cmd.len() > BASH_COMMAND_DISPLAY_MAX_LENGTH {
                format!("Running: {}\u{2026}", &cmd[..BASH_COMMAND_DISPLAY_MAX_LENGTH])
            } else {
                format!("Running: {}", cmd)
            }
        }
        "Glob" => "Searching files".to_string(),
        "Grep" => "Searching code".to_string(),
        "WebFetch" => "Fetching web content".to_string(),
        "WebSearch" => "Searching the web".to_string(),
        "Task" | "Agent" => {
            let desc = input["description"].as_str().unwrap_or("");
            if desc.is_empty() {
                "Running subtask".to_string()
            } else if desc.len() > TASK_DESCRIPTION_DISPLAY_MAX_LENGTH {
                format!(
                    "Subtask: {}\u{2026}",
                    &desc[..TASK_DESCRIPTION_DISPLAY_MAX_LENGTH]
                )
            } else {
                format!("Subtask: {}", desc)
            }
        }
        "AskUserQuestion" => "Waiting for your answer".to_string(),
        "EnterPlanMode" => "Planning".to_string(),
        "NotebookEdit" => "Editing notebook".to_string(),
        _ => format!("Using {}", tool_name),
    }
}

fn process_line(agent: &mut AgentWatchState, line: &str, app: &AppHandle) {
    let record: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return,
    };

    let record_type = record["type"].as_str().unwrap_or("");
    let agent_id = agent.id;

    match record_type {
        "assistant" => {
            let content = match record["message"]["content"].as_array() {
                Some(arr) => arr,
                None => return,
            };
            let has_tool_use = content.iter().any(|b| b["type"].as_str() == Some("tool_use"));

            if has_tool_use {
                agent.is_waiting = false;
                agent.had_tools_in_turn = true;
                let _ = app.emit("agentStatus", json!({"type":"agentStatus","id":agent_id,"status":"active"}));

                let mut has_non_exempt = false;
                for block in content {
                    if block["type"].as_str() == Some("tool_use") {
                        if let Some(tool_id) = block["id"].as_str() {
                            let tool_name = block["name"].as_str().unwrap_or("");
                            let input = &block["input"];
                            let status = format_tool_status(tool_name, input);

                            agent.active_tool_ids.insert(tool_id.to_string());
                            agent.active_tool_names.insert(tool_id.to_string(), tool_name.to_string());
                            agent.active_tool_statuses.insert(tool_id.to_string(), status.clone());

                            if !is_exempt(tool_name) {
                                has_non_exempt = true;
                            }

                            let _ = app.emit("agentToolStart", json!({
                                "type": "agentToolStart",
                                "id": agent_id,
                                "toolId": tool_id,
                                "status": status,
                            }));
                        }
                    }
                }
                if has_non_exempt {
                    agent.permission_sent = false;
                }
            } else {
                let has_text = content.iter().any(|b| b["type"].as_str() == Some("text"));
                if has_text && !agent.had_tools_in_turn {
                    // Text-idle detection handled by the watcher loop
                }
            }
        }
        "user" => {
            let content = &record["message"]["content"];
            if let Some(arr) = content.as_array() {
                let has_tool_result = arr.iter().any(|b| b["type"].as_str() == Some("tool_result"));
                if has_tool_result {
                    for block in arr {
                        if block["type"].as_str() == Some("tool_result") {
                            if let Some(tool_use_id) = block["tool_use_id"].as_str() {
                                // If completed tool was Task/Agent, clear sub-agent tools
                                if agent.active_tool_names.get(tool_use_id).map_or(false, |s| is_subtask_tool(s)) {
                                    agent.active_subagent_tool_ids.remove(tool_use_id);
                                    agent.active_subagent_tool_names.remove(tool_use_id);
                                    let _ = app.emit("subagentClear", json!({
                                        "type": "subagentClear",
                                        "id": agent_id,
                                        "parentToolId": tool_use_id,
                                    }));
                                }

                                agent.active_tool_ids.remove(tool_use_id);
                                agent.active_tool_statuses.remove(tool_use_id);
                                agent.active_tool_names.remove(tool_use_id);

                                let tid = tool_use_id.to_string();
                                let app2 = app.clone();
                                std::thread::spawn(move || {
                                    std::thread::sleep(Duration::from_millis(TOOL_DONE_DELAY_MS));
                                    let _ = app2.emit("agentToolDone", json!({
                                        "type": "agentToolDone",
                                        "id": agent_id,
                                        "toolId": tid,
                                    }));
                                });
                            }
                        }
                    }
                    if agent.active_tool_ids.is_empty() {
                        agent.had_tools_in_turn = false;
                    }
                } else {
                    // New user prompt - new turn
                    agent.clear_activity();
                    let _ = app.emit("agentToolsClear", json!({"type":"agentToolsClear","id":agent_id}));
                    let _ = app.emit("agentStatus", json!({"type":"agentStatus","id":agent_id,"status":"active"}));
                    agent.had_tools_in_turn = false;
                }
            } else if content.is_string() {
                // New user text prompt
                agent.clear_activity();
                let _ = app.emit("agentToolsClear", json!({"type":"agentToolsClear","id":agent_id}));
                let _ = app.emit("agentStatus", json!({"type":"agentStatus","id":agent_id,"status":"active"}));
                agent.had_tools_in_turn = false;
            }
        }
        "system" => {
            if record["subtype"].as_str() == Some("turn_duration") {
                // Definitive turn-end
                if !agent.active_tool_ids.is_empty() {
                    agent.clear_activity();
                    let _ = app.emit("agentToolsClear", json!({"type":"agentToolsClear","id":agent_id}));
                }
                agent.is_waiting = true;
                agent.permission_sent = false;
                agent.had_tools_in_turn = false;
                let _ = app.emit("agentStatus", json!({"type":"agentStatus","id":agent_id,"status":"waiting"}));
            }
        }
        "progress" => {
            process_progress(agent, &record, app);
        }
        _ => {}
    }
}

fn is_subtask_tool(name: &str) -> bool {
    name == "Task" || name == "Agent"
}

fn process_progress(agent: &mut AgentWatchState, record: &Value, app: &AppHandle) {
    let parent_tool_id = match record["parentToolUseID"].as_str() {
        Some(s) => s,
        None => return,
    };
    let data = match record["data"].as_object() {
        Some(d) => d,
        None => return,
    };

    let data_type = data.get("type").and_then(|t| t.as_str()).unwrap_or("");
    let agent_id = agent.id;

    // bash_progress / mcp_progress: tool is executing, not stuck on permission
    if data_type == "bash_progress" || data_type == "mcp_progress" {
        return;
    }

    // If parent tool not tracked (e.g., tool_use was outside replay_tail window),
    // infer it from the progress record and reconstruct state
    if data_type == "agent_progress" && !agent.active_tool_names.contains_key(parent_tool_id) {
        agent.active_tool_ids.insert(parent_tool_id.to_string());
        agent.active_tool_names.insert(parent_tool_id.to_string(), "Agent".to_string());
        agent.active_tool_statuses.insert(parent_tool_id.to_string(), "Running subtask".to_string());
        agent.had_tools_in_turn = true;
        let _ = app.emit("agentStatus", json!({"type":"agentStatus","id":agent_id,"status":"active"}));
        let _ = app.emit("agentToolStart", json!({
            "type": "agentToolStart",
            "id": agent_id,
            "toolId": parent_tool_id,
            "status": "Running subtask",
        }));
    }

    // Only handle agent_progress for Task/Agent tools
    let parent_tool = agent.active_tool_names.get(parent_tool_id).map(|s| s.as_str());
    if !parent_tool.map_or(false, is_subtask_tool) {
        return;
    }

    let msg = match data.get("message").and_then(|m| m.as_object()) {
        Some(m) => m,
        None => return,
    };
    let msg_type = msg.get("type").and_then(|t| t.as_str()).unwrap_or("");
    let inner_msg = match msg.get("message").and_then(|m| m.as_object()) {
        Some(m) => m,
        None => return,
    };
    let content = match inner_msg.get("content").and_then(|c| c.as_array()) {
        Some(arr) => arr,
        None => return,
    };

    if msg_type == "assistant" {
        for block in content {
            if block["type"].as_str() == Some("tool_use") {
                if let Some(tool_id) = block["id"].as_str() {
                    let tool_name = block["name"].as_str().unwrap_or("");
                    let status = format_tool_status(tool_name, &block["input"]);

                    agent.active_subagent_tool_ids
                        .entry(parent_tool_id.to_string())
                        .or_default()
                        .insert(tool_id.to_string());
                    agent.active_subagent_tool_names
                        .entry(parent_tool_id.to_string())
                        .or_default()
                        .insert(tool_id.to_string(), tool_name.to_string());

                    let _ = app.emit("subagentToolStart", json!({
                        "type": "subagentToolStart",
                        "id": agent_id,
                        "parentToolId": parent_tool_id,
                        "toolId": tool_id,
                        "status": status,
                    }));
                }
            }
        }
    } else if msg_type == "user" {
        for block in content {
            if block["type"].as_str() == Some("tool_result") {
                if let Some(tool_use_id) = block["tool_use_id"].as_str() {
                    if let Some(sub_tools) = agent.active_subagent_tool_ids.get_mut(parent_tool_id) {
                        sub_tools.remove(tool_use_id);
                    }
                    if let Some(sub_names) = agent.active_subagent_tool_names.get_mut(parent_tool_id) {
                        sub_names.remove(tool_use_id);
                    }

                    let tid = tool_use_id.to_string();
                    let ptid = parent_tool_id.to_string();
                    let app2 = app.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(TOOL_DONE_DELAY_MS));
                        let _ = app2.emit("subagentToolDone", json!({
                            "type": "subagentToolDone",
                            "id": agent_id,
                            "parentToolId": ptid,
                            "toolId": tid,
                        }));
                    });
                }
            }
        }
    }
}

fn read_new_lines(agent: &mut AgentWatchState, app: &AppHandle) -> bool {
    let metadata = match fs::metadata(&agent.jsonl_path) {
        Ok(m) => m,
        Err(_) => return false,
    };

    let file_size = metadata.len();
    if file_size <= agent.file_offset {
        return false;
    }

    let mut file = match fs::File::open(&agent.jsonl_path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    use std::io::Seek;
    if file.seek(std::io::SeekFrom::Start(agent.file_offset)).is_err() {
        return false;
    }

    let to_read = (file_size - agent.file_offset) as usize;
    let mut buf = vec![0u8; to_read];
    match file.read_exact(&mut buf) {
        Ok(_) => {}
        Err(_) => return false,
    }
    agent.file_offset = file_size;

    let text = agent.line_buffer.clone() + &String::from_utf8_lossy(&buf);
    let mut lines: Vec<&str> = text.split('\n').collect();
    agent.line_buffer = lines.pop().unwrap_or("").to_string();

    let has_data = lines.iter().any(|l| !l.trim().is_empty());
    if has_data {
        // New data arriving -- clear permission state
        if agent.permission_sent {
            agent.permission_sent = false;
            let _ = app.emit("agentToolPermissionClear", json!({"type":"agentToolPermissionClear","id":agent.id}));
        }
    }

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        process_line(agent, trimmed, app);
    }

    has_data
}

/// Shared state for the JSONL watcher
pub struct JsonlWatcherState {
    agents: HashMap<i32, AgentWatchState>,
    /// JSONL paths we're waiting for (not yet created)
    pending: HashMap<i32, PathBuf>,
    /// Ticks since last data for permission timer
    no_data_ticks: HashMap<i32, u64>,
    /// Ticks since last data for text-idle timer
    text_idle_ticks: HashMap<i32, u64>,
}

pub type SharedJsonlWatcher = Arc<Mutex<JsonlWatcherState>>;

pub fn new_shared() -> SharedJsonlWatcher {
    Arc::new(Mutex::new(JsonlWatcherState {
        agents: HashMap::new(),
        pending: HashMap::new(),
        no_data_ticks: HashMap::new(),
        text_idle_ticks: HashMap::new(),
    }))
}

/// Register an agent for JSONL watching.
/// `project_dir` is the Claude project directory for this agent's CWD.
/// Reads the tail of the file to establish the current turn state.
pub fn register_agent(
    watcher: &SharedJsonlWatcher,
    agent_id: i32,
    session_uuid: &str,
    project_dir: &Path,
    app: &AppHandle,
) {
    let mut state = watcher.lock().unwrap();
    let jsonl_path = project_dir.join(format!("{}.jsonl", session_uuid));

    if jsonl_path.exists() {
        let mut agent = AgentWatchState::new(agent_id, jsonl_path);
        if let Ok(meta) = fs::metadata(&agent.jsonl_path) {
            let file_size = meta.len();
            // Read the tail of the file to catch the current turn state
            replay_tail(&mut agent, file_size, app);
            agent.file_offset = file_size;
        }
        state.agents.insert(agent_id, agent);
    } else {
        state.pending.insert(agent_id, jsonl_path);
    }
}

/// Read the last TAIL_READ_BYTES of a JSONL file and replay through process_line
/// to establish the current turn's tool/status state.
fn replay_tail(agent: &mut AgentWatchState, file_size: u64, app: &AppHandle) {
    use std::io::Seek;

    let mut file = match fs::File::open(&agent.jsonl_path) {
        Ok(f) => f,
        Err(_) => return,
    };

    let start = if file_size > TAIL_READ_BYTES {
        file_size - TAIL_READ_BYTES
    } else {
        0
    };

    if file.seek(std::io::SeekFrom::Start(start)).is_err() {
        return;
    }

    let to_read = (file_size - start) as usize;
    let mut buf = vec![0u8; to_read];
    if file.read_exact(&mut buf).is_err() {
        return;
    }

    let text = String::from_utf8_lossy(&buf);
    let lines: Vec<&str> = text.split('\n').collect();

    // If we started mid-file, skip the first (potentially partial) line
    let skip = if start > 0 { 1 } else { 0 };

    // Find the last user prompt (new turn boundary) so we only replay the current turn
    let mut turn_start_idx = skip;
    for (i, line) in lines.iter().enumerate().skip(skip) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(record) = serde_json::from_str::<serde_json::Value>(trimmed) {
            let record_type = record["type"].as_str().unwrap_or("");
            if record_type == "user" {
                let content = &record["message"]["content"];
                let is_new_prompt = content.is_string()
                    || content
                        .as_array()
                        .map_or(false, |arr| !arr.iter().any(|b| b["type"].as_str() == Some("tool_result")));
                if is_new_prompt {
                    turn_start_idx = i;
                }
            }
            // turn_duration also marks a turn boundary
            if record_type == "system" && record["subtype"].as_str() == Some("turn_duration") {
                turn_start_idx = i;
            }
        }
    }

    // Replay from the last turn boundary
    for line in lines.iter().skip(turn_start_idx) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        process_line(agent, trimmed, app);
    }
}

/// Unregister an agent from JSONL watching.
pub fn unregister_agent(watcher: &SharedJsonlWatcher, agent_id: i32) {
    let mut state = watcher.lock().unwrap();
    state.agents.remove(&agent_id);
    state.pending.remove(&agent_id);
    state.no_data_ticks.remove(&agent_id);
    state.text_idle_ticks.remove(&agent_id);
}

/// Start the polling loop. Call this once after creating the watcher.
pub fn start_poll_loop(watcher: SharedJsonlWatcher, app: AppHandle) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(JSONL_POLL_INTERVAL_MS));

            let mut state = watcher.lock().unwrap();

            // Check pending files (waiting for JSONL creation)
            let mut newly_created = Vec::new();
            for (&agent_id, path) in &state.pending {
                if path.exists() {
                    newly_created.push((agent_id, path.clone()));
                }
            }
            for (agent_id, path) in newly_created {
                state.pending.remove(&agent_id);
                state.agents.insert(agent_id, AgentWatchState::new(agent_id, path));
            }

            // Read new lines for each agent
            let agent_ids: Vec<i32> = state.agents.keys().copied().collect();
            for agent_id in agent_ids {
                let agent = match state.agents.get_mut(&agent_id) {
                    Some(a) => a,
                    None => continue,
                };
                let had_data = read_new_lines(agent, &app);

                if had_data {
                    state.no_data_ticks.remove(&agent_id);
                    state.text_idle_ticks.remove(&agent_id);
                } else {
                    // Permission timer: count ticks without data
                    let agent = state.agents.get(&agent_id).unwrap();
                    if agent.has_non_exempt_tools() && !agent.permission_sent {
                        let ticks = state.no_data_ticks.entry(agent_id).or_insert(0);
                        *ticks += JSONL_POLL_INTERVAL_MS;
                        if *ticks >= PERMISSION_TIMER_DELAY_MS {
                            state.no_data_ticks.remove(&agent_id);
                            let agent = state.agents.get_mut(&agent_id).unwrap();
                            agent.permission_sent = true;

                            // Find stuck sub-agent parent tool IDs
                            let mut stuck_parents = Vec::new();
                            for (parent_id, sub_names) in &agent.active_subagent_tool_names {
                                for tool_name in sub_names.values() {
                                    if !is_exempt(tool_name) {
                                        stuck_parents.push(parent_id.clone());
                                        break;
                                    }
                                }
                            }

                            let _ = app.emit("agentToolPermission", json!({
                                "type": "agentToolPermission",
                                "id": agent_id,
                            }));
                            for parent_id in stuck_parents {
                                let _ = app.emit("subagentToolPermission", json!({
                                    "type": "subagentToolPermission",
                                    "id": agent_id,
                                    "parentToolId": parent_id,
                                }));
                            }
                        }
                    }

                    // Text-idle timer: only if no tools used in this turn
                    let agent = state.agents.get(&agent_id).unwrap();
                    if !agent.had_tools_in_turn && !agent.is_waiting {
                        let ticks = state.text_idle_ticks.entry(agent_id).or_insert(0);
                        *ticks += JSONL_POLL_INTERVAL_MS;
                        if *ticks >= TEXT_IDLE_DELAY_MS {
                            state.text_idle_ticks.remove(&agent_id);
                            let agent = state.agents.get_mut(&agent_id).unwrap();
                            agent.is_waiting = true;
                            let _ = app.emit("agentStatus", json!({
                                "type": "agentStatus",
                                "id": agent_id,
                                "status": "waiting",
                            }));
                        }
                    }
                }
            }

            drop(state);
        }
    });
}

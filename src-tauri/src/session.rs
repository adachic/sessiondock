use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use sysinfo::System;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionStatus {
    Running,
    Waiting,
    Done,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    pub pid: u32,
    pub cwd: String,
    pub project_name: String,
    pub started_at: u64,
    pub status: SessionStatus,
    pub topic: String,
    pub last_message: String,
    pub current_task: String,
    pub model: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub last_context_used: u64,
    pub estimated_cost: f64,
    pub elapsed_seconds: u64,
    pub status_since_seconds: u64,
}

#[derive(Debug, Deserialize)]
struct SessionFile {
    pid: u32,
    #[serde(rename = "sessionId")]
    session_id: String,
    cwd: String,
    #[serde(rename = "startedAt")]
    started_at: u64,
}

pub struct SessionManager {
    claude_dir: PathBuf,
    system: System,
    session_log_state: HashMap<String, LogState>,
}

struct LogState {
    file_pos: u64,
    tokens_in: u64,
    tokens_out: u64,
    model: String,
    status: SessionStatus,
    status_changed_at: u64,
    topic: String,
    last_message: String,
    current_task: String,
    last_context_used: u64,
}

impl SessionManager {
    pub fn new() -> Self {
        let home = dirs::home_dir().expect("Cannot find home directory");
        let claude_dir = home.join(".claude");
        Self {
            claude_dir,
            system: System::new(),
            session_log_state: HashMap::new(),
        }
    }

    pub fn get_sessions(&mut self) -> Vec<Session> {
        let sessions_dir = self.claude_dir.join("sessions");
        if !sessions_dir.exists() {
            return vec![];
        }

        self.system
            .refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let mut sessions = Vec::new();

        let entries = match fs::read_dir(&sessions_dir) {
            Ok(e) => e,
            Err(_) => return vec![],
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                if let Some(session) = self.parse_session_file(&path, now_ms) {
                    sessions.push(session);
                }
            }
        }

        sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        sessions
    }

    fn parse_session_file(&mut self, path: &Path, now_ms: u64) -> Option<Session> {
        let content = fs::read_to_string(path).ok()?;
        let sf: SessionFile = serde_json::from_str(&content).ok()?;

        let pid = sysinfo::Pid::from_u32(sf.pid);
        // PIDが存在し、かつClaude Code関連プロセス(node/claude)であることを確認
        // PIDが別プロセスに再利用されている場合を除外する
        let is_alive = self
            .system
            .process(pid)
            .map(|p| {
                let name = p.name().to_string_lossy().to_lowercase();
                name.contains("claude") || name.contains("node")
            })
            .unwrap_or(false);

        let project_name = Path::new(&sf.cwd)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| sf.cwd.clone());

        let elapsed_seconds = (now_ms.saturating_sub(sf.started_at)) / 1000;

        self.update_log_state(&sf.session_id, &sf.cwd, now_ms);

        let state = self.session_log_state.get(&sf.session_id);
        let (topic, last_message, current_task, model, status_from_log, tokens_in, tokens_out, last_context_used, status_changed_at) =
            match state {
                Some(s) => (
                    s.topic.clone(),
                    s.last_message.clone(),
                    s.current_task.clone(),
                    s.model.clone(),
                    s.status.clone(),
                    s.tokens_in,
                    s.tokens_out,
                    s.last_context_used,
                    s.status_changed_at,
                ),
                None => (
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    SessionStatus::Running,
                    0,
                    0,
                    0,
                    sf.started_at,
                ),
            };

        let status = if !is_alive {
            SessionStatus::Done
        } else {
            status_from_log
        };

        let status_since_seconds = now_ms.saturating_sub(status_changed_at) / 1000;
        let estimated_cost = estimate_cost(&model, tokens_in, tokens_out);

        Some(Session {
            session_id: sf.session_id,
            pid: sf.pid,
            cwd: sf.cwd,
            project_name,
            started_at: sf.started_at,
            status,
            topic,
            last_message,
            current_task,
            model,
            tokens_in,
            tokens_out,
            last_context_used,
            estimated_cost,
            elapsed_seconds,
            status_since_seconds,
        })
    }

    fn update_log_state(&mut self, session_id: &str, cwd: &str, now_ms: u64) {
        let projects_dir = self.claude_dir.join("projects");
        if !projects_dir.exists() {
            return;
        }

        let encoded_cwd = cwd.replace('/', "-");
        let project_dir = projects_dir.join(&encoded_cwd);
        let jsonl_path = project_dir.join(format!("{}.jsonl", session_id));

        if !jsonl_path.exists() {
            return;
        }

        let state = self
            .session_log_state
            .entry(session_id.to_string())
            .or_insert_with(|| LogState {
                file_pos: 0,
                tokens_in: 0,
                tokens_out: 0,
                model: String::new(),
                status: SessionStatus::Running,
                status_changed_at: now_ms,
                topic: String::new(),
                last_message: String::new(),
                current_task: String::new(),
                last_context_used: 0,
            });

        let file = match fs::File::open(&jsonl_path) {
            Ok(f) => f,
            Err(_) => return,
        };

        let mut reader = BufReader::new(file);
        if state.file_pos > 0 {
            let _ = reader.seek(SeekFrom::Start(state.file_pos));
        }

        let mut current_pos = state.file_pos;
        let mut line = String::new();

        while reader.read_line(&mut line).unwrap_or(0) > 0 {
            current_pos += line.len() as u64;

            if let Ok(raw) = serde_json::from_str::<serde_json::Value>(&line) {
                // Skip sidechain entries (subagents)
                if raw
                    .get("isSidechain")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    // Still count tokens for cost
                    if let Some(msg) = raw.get("message") {
                        if let Some(usage) = msg.get("usage") {
                            accumulate_tokens(usage, &mut state.tokens_in, &mut state.tokens_out);
                        }
                    }
                    line.clear();
                    continue;
                }

                // Get timestamp from entry
                let entry_ts = raw
                    .get("timestamp")
                    .and_then(|v| v.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.timestamp_millis() as u64)
                    .unwrap_or(now_ms);

                let entry_type = raw.get("type").and_then(|v| v.as_str()).unwrap_or("");

                match entry_type {
                    "assistant" => {
                        // Check stop_reason to determine actual status:
                        // "end_turn" = Claude finished, truly waiting for user input
                        // "tool_use" = Claude is executing tools, still working
                        let stop_reason = raw
                            .get("message")
                            .and_then(|m| m.get("stop_reason"))
                            .and_then(|v| v.as_str());

                        let new_status = match stop_reason {
                            Some("tool_use") => SessionStatus::Running,
                            Some("end_turn") => SessionStatus::Waiting,
                            // stop_reason が null/None → Claudeの最終応答（入力待ち）
                            // ストリーミング途中は stop_reason が無いが、
                            // JSONLには完了したメッセージのみ記録されるので Waiting で正しい
                            None | Some("") => SessionStatus::Waiting,
                            _ => SessionStatus::Running,
                        };

                        let old_status = state.status.clone();
                        state.status = new_status.clone();
                        if old_status != new_status {
                            state.status_changed_at = entry_ts;
                        }

                        if let Some(msg) = raw.get("message") {
                            if let Some(m) = msg.get("model").and_then(|v| v.as_str()) {
                                state.model = m.to_string();
                            }

                            if let Some(usage) = msg.get("usage") {
                                accumulate_tokens(
                                    usage,
                                    &mut state.tokens_in,
                                    &mut state.tokens_out,
                                );
                                // 直近リクエストのコンテキスト使用量を記録
                                let ctx: u64 = usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0)
                                    + usage.get("cache_creation_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0)
                                    + usage.get("cache_read_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                                if ctx > 0 {
                                    state.last_context_used = ctx;
                                }
                            }

                            // Extract context text
                            if let Some(content) = msg.get("content").and_then(|v| v.as_array()) {
                                // Look for tool_use (task info) first
                                for item in content {
                                    if item.get("type").and_then(|v| v.as_str())
                                        == Some("tool_use")
                                    {
                                        let tool_name = item
                                            .get("name")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("");
                                        if let Some(input) = item.get("input") {
                                            extract_task_info(
                                                tool_name,
                                                input,
                                                &mut state.current_task,
                                            );
                                        }
                                    }
                                }

                                // Extract last text as message
                                for item in content.iter().rev() {
                                    if item.get("type").and_then(|v| v.as_str()) == Some("text") {
                                        if let Some(text) =
                                            item.get("text").and_then(|v| v.as_str())
                                        {
                                            let ctx = extract_context(text);
                                            if !ctx.is_empty() {
                                                state.last_message = ctx;
                                            }
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    "user" => {
                        // "user" type には2種類ある:
                        // 1. 本当のユーザー入力 (content が文字列 or "text" type)
                        //    → Running (Claudeがこれから処理する)
                        // 2. ツール実行結果の返却 (content に "tool_result" が含まれる)
                        //    → 状態変更なし (Claudeはまだ作業の途中)
                        let is_real_user_input = is_actual_user_message(&raw);

                        if is_real_user_input {
                            let old_status = state.status.clone();
                            state.status = SessionStatus::Running;
                            if old_status != SessionStatus::Running {
                                state.status_changed_at = entry_ts;
                            }

                            let user_text = extract_user_text(&raw);

                            if state.topic.is_empty() && !user_text.is_empty() {
                                state.topic = user_text.chars().take(120).collect();
                            }

                            if !user_text.is_empty() {
                                state.last_message = user_text.chars().take(120).collect();
                            }
                        }
                        // tool_result の場合は状態を変更しない（Claudeはまだ作業中）
                    }
                    "system" => {
                        // system (turn_duration等) → ターン終了 = 入力待ち確定
                        let subtype = raw.get("subtype").and_then(|v| v.as_str()).unwrap_or("");
                        if subtype == "turn_duration" {
                            let old_status = state.status.clone();
                            state.status = SessionStatus::Waiting;
                            if old_status != SessionStatus::Waiting {
                                state.status_changed_at = entry_ts;
                            }
                        }
                    }
                    _ => {
                        // progress / tool-result — don't change status
                    }
                }
            }
            line.clear();
        }

        state.file_pos = current_pos;
    }
}

fn is_actual_user_message(raw: &serde_json::Value) -> bool {
    if let Some(msg) = raw.get("message") {
        if let Some(content) = msg.get("content") {
            // content が文字列 → 本当のユーザー入力
            if content.is_string() {
                return true;
            }
            // content が配列 → 中身を確認
            if let Some(arr) = content.as_array() {
                for item in arr {
                    let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    // "text" があれば本当のユーザー入力
                    if item_type == "text" {
                        return true;
                    }
                    // "tool_result" だけならツール結果の返却
                }
                return false;
            }
        }
    }
    false
}

fn extract_user_text(raw: &serde_json::Value) -> String {
    if let Some(msg) = raw.get("message") {
        if let Some(content) = msg.get("content") {
            if let Some(text) = content.as_str() {
                return extract_context(text);
            }
            if let Some(arr) = content.as_array() {
                for item in arr {
                    if item.get("type").and_then(|v| v.as_str()) == Some("text") {
                        if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                            return extract_context(text);
                        }
                    }
                }
            }
        }
    }
    String::new()
}

fn accumulate_tokens(usage: &serde_json::Value, total_in: &mut u64, total_out: &mut u64) {
    if let Some(v) = usage.get("input_tokens").and_then(|v| v.as_u64()) {
        *total_in += v;
    }
    if let Some(v) = usage
        .get("cache_creation_input_tokens")
        .and_then(|v| v.as_u64())
    {
        *total_in += v;
    }
    if let Some(v) = usage
        .get("cache_read_input_tokens")
        .and_then(|v| v.as_u64())
    {
        *total_in += v;
    }
    if let Some(v) = usage.get("output_tokens").and_then(|v| v.as_u64()) {
        *total_out += v;
    }
}

fn extract_context(text: &str) -> String {
    let mut lines: Vec<&str> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Skip markdown formatting noise
        if trimmed.starts_with("```") || trimmed.starts_with("---") || trimmed.starts_with("| ") {
            continue;
        }
        let clean = trimmed.trim_start_matches('#').trim();
        if clean.is_empty() {
            continue;
        }
        lines.push(clean);
        if lines.len() >= 2 {
            break;
        }
    }
    lines.join(" / ")
}

fn extract_task_info(tool_name: &str, input: &serde_json::Value, current_task: &mut String) {
    match tool_name {
        "TaskCreate" => {
            if let Some(subject) = input.get("subject").and_then(|v| v.as_str()) {
                *current_task = subject.to_string();
            }
        }
        "TaskUpdate" => {
            if let Some(active) = input.get("activeForm").and_then(|v| v.as_str()) {
                *current_task = active.to_string();
            }
        }
        "Write" | "Edit" => {
            if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                let filename = Path::new(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if !filename.is_empty() {
                    *current_task = format!("Editing: {}", filename);
                }
            }
        }
        "Bash" => {
            if let Some(desc) = input.get("description").and_then(|v| v.as_str()) {
                *current_task = desc.chars().take(80).collect();
            }
        }
        _ => {}
    }
}

fn estimate_cost(model: &str, tokens_in: u64, tokens_out: u64) -> f64 {
    let (price_in, price_out) = match model {
        m if m.contains("opus") => (5.0, 25.0),
        m if m.contains("sonnet") => (3.0, 15.0),
        m if m.contains("haiku") => (1.0, 5.0),
        _ => (3.0, 15.0),
    };
    (tokens_in as f64 * price_in + tokens_out as f64 * price_out) / 1_000_000.0
}

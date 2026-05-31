use std::io::{BufRead, BufReader, Seek, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use crate::child;
use crate::cleanup;
use crate::interactive_providers;
use crate::protocol;
use crate::providers::ProviderKind;
use crate::sanitize;

const DEFAULT_IDLE_TIMEOUT_MS: u64 = 600_000;
const DEFAULT_HARD_TIMEOUT_MS: u64 = 3_600_000;
const COMPLETION_QUIESCE_MS: u64 = 3_000;
const POLL_INTERVAL_MS: u64 = 200;

pub struct InteractiveConfig {
    pub provider: ProviderKind,
    pub provider_bin: String,
    pub prompt: String,
    pub cwd: PathBuf,
    pub cols: u16,
    pub rows: u16,
    pub idle_timeout_ms: u64,
    pub hard_timeout_ms: u64,
    pub resume_session: Option<String>,
    pub model: Option<String>,
    pub extra_args: Vec<String>,
    pub show_session_footer: bool,
    pub output_format: String,
    pub emit_runtime_events: bool,
}

impl InteractiveConfig {
    pub fn new(
        provider: ProviderKind,
        provider_bin: String,
        prompt: String,
        cwd: PathBuf,
        resume_session: Option<String>,
        model: Option<String>,
        extra_args: Vec<String>,
        show_session_footer: bool,
        output_format: String,
    ) -> Self {
        Self {
            provider,
            provider_bin,
            prompt,
            cwd,
            cols: 120,
            rows: 40,
            idle_timeout_ms: DEFAULT_IDLE_TIMEOUT_MS,
            hard_timeout_ms: DEFAULT_HARD_TIMEOUT_MS,
            resume_session,
            model,
            extra_args,
            show_session_footer,
            output_format,
            emit_runtime_events: true,
        }
    }
}

pub fn run_interactive(config: InteractiveConfig) -> i32 {
    let run_id = format!("run_{}", &uuid::Uuid::new_v4().to_string()[..8]);

    if config.emit_runtime_events {
        protocol::emit_runtime_started(&run_id, env!("CARGO_PKG_VERSION"));
    }

    let prompt = match sanitize::sanitize_prompt(&config.prompt) {
        Ok(p) => p,
        Err(e) => {
            emit_error(&run_id, &config, &format!("prompt rejected: {e}"), 16);
            return 16;
        }
    };

    let positional_prompt = if interactive_providers::accepts_positional_prompt(config.provider)
        && config.resume_session.is_none()
    {
        Some(prompt.as_str())
    } else {
        None
    };

    let tui_args = interactive_providers::build_interactive_args(
        config.provider,
        positional_prompt,
        config.resume_session.as_deref(),
        config.model.as_deref(),
        &config.extra_args,
    );

    let before_files = interactive_providers::list_session_files(config.provider, &config.cwd);
    let started_at_ms = epoch_ms().saturating_sub(1_000);

    let stop = Arc::new(AtomicBool::new(false));
    let _ = signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&stop));
    let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&stop));

    // Kiro hangs in PTY — use pipe spawn with stdout parsing instead
    if config.provider == ProviderKind::Kiro {
        return run_kiro_pipe(
            &config,
            &run_id,
            &tui_args,
            &stop,
            started_at_ms,
            &before_files,
        );
    }

    let mut pty_child = match child::PtyChild::spawn(
        &config.provider_bin,
        &tui_args,
        &config.cwd,
        config.cols,
        config.rows,
        Arc::clone(&stop),
    ) {
        Ok(c) => c,
        Err(e) => {
            emit_error(&run_id, &config, &format!("spawn failed: {e}"), 4);
            return 4;
        }
    };

    let child_pid = pty_child.child.process_id().unwrap_or(0);
    if config.emit_runtime_events {
        protocol::emit_provider_spawned(&run_id, config.provider.id(), child_pid);
    }

    pty_child.wait_quiescence(1500);

    // Inject prompt
    if positional_prompt.is_none() {
        let (paste_bytes, submit_bytes) = sanitize::bracketed_paste(&prompt);
        if let Err(e) = inject_prompt(&pty_child, &paste_bytes, &submit_bytes) {
            emit_error(&run_id, &config, &format!("injection failed: {e}"), 4);
            cleanup::kill_process_group(child_pid, &run_id, config.emit_runtime_events);
            return 4;
        }
    }

    if config.emit_runtime_events {
        protocol::emit_prompt_injected(&run_id);
    }

    // Tail session file + completion detection
    let last_activity = Arc::new(AtomicU64::new(epoch_ms()));
    let active_tools = Arc::new(AtomicUsize::new(0));

    let exit_code = wait_and_tail(
        &config,
        &run_id,
        &mut pty_child,
        child_pid,
        &stop,
        &last_activity,
        &active_tools,
        started_at_ms,
    );

    // Footer
    if config.show_session_footer {
        emit_footer(&config, &before_files, started_at_ms);
    }

    pty_child.join_drain();
    exit_code
}

fn inject_prompt(
    pty_child: &child::PtyChild,
    paste_bytes: &[u8],
    submit_bytes: &[u8],
) -> Result<(), String> {
    {
        let mut w = pty_child.writer.lock().map_err(|_| "lock poisoned")?;
        w.write_all(paste_bytes).map_err(|e| e.to_string())?;
        let _ = w.flush();
    }
    std::thread::sleep(std::time::Duration::from_millis(150));
    {
        let mut w = pty_child.writer.lock().map_err(|_| "lock poisoned")?;
        w.write_all(submit_bytes).map_err(|e| e.to_string())?;
        let _ = w.flush();
    }
    Ok(())
}

/// Combined wait loop: polls for completion while tailing the session file for output.
fn wait_and_tail(
    config: &InteractiveConfig,
    run_id: &str,
    pty_child: &mut child::PtyChild,
    child_pid: u32,
    stop: &Arc<AtomicBool>,
    last_activity: &Arc<AtomicU64>,
    active_tools: &Arc<AtomicUsize>,
    started_at_ms: u64,
) -> i32 {
    let start = std::time::Instant::now();
    let mut last_pty_change: u64 = epoch_ms();
    let mut session_file: Option<PathBuf> = None;
    let mut file_offset: u64 = 0;
    let mut completion_candidate_since: Option<u64> = None;
    let mut last_assistant: Option<serde_json::Value> = None;
    let mut stdout = std::io::stdout().lock();

    loop {
        if stop.load(Ordering::Relaxed) {
            if config.emit_runtime_events {
                protocol::emit_interrupted(run_id, "");
            }
            cleanup::graceful_exit(
                &pty_child.writer,
                &mut pty_child.child,
                child_pid,
                run_id,
                config.emit_runtime_events,
            );
            return 2;
        }

        if let Ok(Some(status)) = pty_child.child.try_wait() {
            // Final drain of session file
            if let Some(ref path) = session_file {
                drain_session_file(
                    path,
                    &mut file_offset,
                    config,
                    &mut last_assistant,
                    active_tools,
                    &mut stdout,
                );
            }
            emit_result(config, &last_assistant, &mut stdout);
            return if status.success() { 0 } else { 1 };
        }

        // PTY activity tracking (for idle timeout only, not completion)
        let current_change = pty_child.last_change_us.load(Ordering::Relaxed);
        if current_change != last_pty_change {
            last_pty_change = current_change;
            // For agy (no session file), PTY activity IS the activity signal
            if config.provider == ProviderKind::Agy {
                last_activity.store(epoch_ms(), Ordering::Relaxed);
                completion_candidate_since = None;
            }
        }

        // Discover session file
        if session_file.is_none() {
            if let Some(dir) =
                interactive_providers::resolve_session_path(config.provider, &config.cwd)
            {
                let found = if config.provider == ProviderKind::Grok {
                    interactive_providers::find_newest_jsonl_named(
                        &dir,
                        started_at_ms,
                        "chat_history",
                    )
                } else {
                    interactive_providers::find_newest_jsonl(&dir, started_at_ms)
                };
                if let Some(path) = found {
                    file_offset = 0; // New session file — read from beginning
                    session_file = Some(path);
                    if config.emit_runtime_events {
                        protocol::emit_session_started(
                            run_id,
                            "",
                            &session_file.as_ref().unwrap().display().to_string(),
                        );
                    }
                }
            }
        }

        // Tail session file for new content
        if let Some(ref path) = session_file {
            let new_len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            if new_len > file_offset {
                drain_session_file(
                    path,
                    &mut file_offset,
                    config,
                    &mut last_assistant,
                    active_tools,
                    &mut stdout,
                );
                last_activity.store(epoch_ms(), Ordering::Relaxed);
                completion_candidate_since = None;
            }
        }

        // Completion detection: session file stable + has assistant response
        let now = epoch_ms();
        let file_idle_ms = now.saturating_sub(last_activity.load(Ordering::Relaxed));
        // Agy uses protobuf (no JSONL tail) — detect completion via PTY quiescence alone
        let has_response = if config.provider == ProviderKind::Agy {
            // For agy, consider "has response" if PTY has been active then went quiet
            last_pty_change > 0 && file_idle_ms >= COMPLETION_QUIESCE_MS
        } else {
            last_assistant.is_some()
        };
        if file_idle_ms >= COMPLETION_QUIESCE_MS && has_response {
            match completion_candidate_since {
                None => completion_candidate_since = Some(now),
                Some(since) if now.saturating_sub(since) >= COMPLETION_QUIESCE_MS => {
                    // Final drain
                    if let Some(ref path) = session_file {
                        drain_session_file(
                            path,
                            &mut file_offset,
                            config,
                            &mut last_assistant,
                            active_tools,
                            &mut stdout,
                        );
                    }
                    emit_result(config, &last_assistant, &mut stdout);
                    if config.emit_runtime_events {
                        protocol::emit_stop_received(run_id, "");
                    }
                    send_exit_signal(pty_child, child_pid);
                    return 0;
                }
                _ => {}
            }
        }

        // Idle timeout (skip while tools active)
        if file_idle_ms > config.idle_timeout_ms && active_tools.load(Ordering::Relaxed) == 0 {
            emit_error(
                run_id,
                config,
                &format!("idle timeout: {}ms", config.idle_timeout_ms),
                6,
            );
            cleanup::kill_process_group(child_pid, run_id, config.emit_runtime_events);
            return 6;
        }

        // Hard timeout
        if start.elapsed() > std::time::Duration::from_millis(config.hard_timeout_ms) {
            emit_error(
                run_id,
                config,
                &format!("hard timeout: {}ms", config.hard_timeout_ms),
                6,
            );
            cleanup::kill_process_group(child_pid, run_id, config.emit_runtime_events);
            return 6;
        }

        std::thread::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS));
    }
}

/// Read new lines from session file, normalize, and emit.
fn drain_session_file(
    path: &PathBuf,
    offset: &mut u64,
    config: &InteractiveConfig,
    last_assistant: &mut Option<serde_json::Value>,
    active_tools: &Arc<AtomicUsize>,
    stdout: &mut std::io::StdoutLock<'_>,
) {
    let Ok(file) = std::fs::File::open(path) else {
        return;
    };
    let mut reader = BufReader::new(file);
    if reader.seek(std::io::SeekFrom::Start(*offset)).is_err() {
        return;
    }

    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(n) => {
                *offset += n as u64;
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                process_session_line(trimmed, config, last_assistant, active_tools, stdout);
            }
            Err(_) => break,
        }
    }
}

/// Normalize and emit a single session JSONL line.
fn process_session_line(
    line: &str,
    config: &InteractiveConfig,
    last_assistant: &mut Option<serde_json::Value>,
    active_tools: &Arc<AtomicUsize>,
    stdout: &mut std::io::StdoutLock<'_>,
) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return;
    };

    let normalized = normalize_provider_line(config.provider, &value);
    let Some(normalized) = normalized else {
        return;
    };

    // Track assistant messages for result synthesis
    let record_type = normalized
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("");
    if record_type == "assistant" {
        *last_assistant = Some(normalized.clone());
    }

    // Track tool state
    update_tool_state(&normalized, active_tools);

    // Emit based on output format
    match config.output_format.as_str() {
        "stream-json" => {
            if let Ok(json) = serde_json::to_string(&normalized) {
                let _ = writeln!(stdout, "{json}");
                let _ = stdout.flush();
            }
        }
        "json" => {
            // Only emit result at the end (handled by emit_result)
        }
        _ => {
            // text: print assistant text content only
            if record_type == "assistant" {
                if let Some(text) = extract_text_content(&normalized) {
                    if !text.is_empty() {
                        let _ = write!(stdout, "{text}");
                        let _ = stdout.flush();
                    }
                }
            }
        }
    }
}

use crate::interactive_normalize::{
    emit_result, extract_text_content, normalize_provider_line, update_tool_state,
};

fn send_exit_signal(pty_child: &mut child::PtyChild, child_pid: u32) {
    if let Ok(mut w) = pty_child.writer.lock() {
        let _ = w.write_all(b"\x03");
        let _ = w.flush();
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(3_000);
    loop {
        if let Ok(Some(_)) = pty_child.child.try_wait() {
            return;
        }
        if std::time::Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    cleanup::kill_process_group(child_pid, "interactive", false);
}

fn emit_footer(
    config: &InteractiveConfig,
    before_files: &std::collections::HashSet<PathBuf>,
    started_at_ms: u64,
) {
    // Agy: read session ID from last_conversations.json (cwd → uuid mapping)
    if config.provider == ProviderKind::Agy {
        if let Some(id) = resolve_agy_conversation_id(&config.cwd) {
            interactive_providers::emit_session_footer(config.provider, &id);
        }
        return;
    }

    let after_files = interactive_providers::list_session_files(config.provider, &config.cwd);
    let new_files: Vec<_> = after_files.difference(before_files).collect();

    let session_id = if let Some(ref resume_id) = config.resume_session {
        Some(resume_id.clone())
    } else if let Some(new_file) = new_files.first() {
        interactive_providers::extract_session_id(config.provider, new_file)
    } else {
        interactive_providers::resolve_session_path(config.provider, &config.cwd)
            .and_then(|dir| interactive_providers::find_newest_jsonl(&dir, started_at_ms))
            .and_then(|path| interactive_providers::extract_session_id(config.provider, &path))
    };

    if let Some(id) = session_id {
        interactive_providers::emit_session_footer(config.provider, &id);
    }
}

/// Read agy conversation ID for a cwd from ~/.gemini/antigravity-cli/cache/last_conversations.json
fn resolve_agy_conversation_id(cwd: &std::path::Path) -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let path = std::path::PathBuf::from(home)
        .join(".gemini")
        .join("antigravity-cli")
        .join("cache")
        .join("last_conversations.json");
    let content = std::fs::read_to_string(&path).ok()?;
    let map: serde_json::Value = serde_json::from_str(&content).ok()?;
    let cwd_str = cwd.to_string_lossy();
    map.get(cwd_str.as_ref())
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn emit_error(run_id: &str, config: &InteractiveConfig, message: &str, code: i32) {
    if config.emit_runtime_events {
        protocol::emit_error(run_id, message, code);
    } else {
        eprintln!("ai-e: {message}");
    }
}

fn epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Strip ANSI escape sequences from text.
fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip ESC sequence
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                // Skip until we hit a letter (the terminator)
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            } else {
                // Other ESC sequences — skip next char
                chars.next();
            }
        } else {
            result.push(c);
        }
    }
    // Also strip common prompt prefixes like "> "
    let trimmed = result.trim_start_matches("> ").trim();
    trimmed.to_string()
}

/// Kiro-specific pipe-based interactive path with full parsing.
/// Kiro hangs in PTY, so we use pipe spawn and parse stdout JSONL directly.
fn run_kiro_pipe(
    config: &InteractiveConfig,
    run_id: &str,
    args: &[String],
    stop: &Arc<AtomicBool>,
    started_at_ms: u64,
    _before_files: &std::collections::HashSet<PathBuf>,
) -> i32 {
    use std::process::{Command, Stdio};

    let mut cmd = Command::new(&config.provider_bin);
    cmd.args(args);
    cmd.current_dir(&config.cwd);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            emit_error(run_id, config, &format!("kiro spawn failed: {e}"), 4);
            return 4;
        }
    };

    let child_pid = child.id();
    if config.emit_runtime_events {
        protocol::emit_provider_spawned(run_id, "kiro", child_pid);
        protocol::emit_prompt_injected(run_id);
    }

    let mut last_assistant: Option<serde_json::Value> = None;
    let mut stdout_handle = child.stdout.take();
    let _active_tools = Arc::new(AtomicUsize::new(0));
    let mut out = std::io::stdout().lock();

    // Read stdout line by line, normalize, emit
    if let Some(ref mut reader) = stdout_handle {
        let buf_reader = BufReader::new(reader);
        for line_result in buf_reader.lines() {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let Ok(line) = line_result else { break };
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            // Kiro outputs ANSI-escaped text, not JSONL. Strip escapes and emit.
            let clean = strip_ansi(trimmed);
            if clean.is_empty() {
                continue;
            }
            match config.output_format.as_str() {
                "stream-json" => {
                    let event = serde_json::json!({
                        "type": "assistant",
                        "message": {"content": clean, "role": "assistant"},
                    });
                    if let Ok(json) = serde_json::to_string(&event) {
                        let _ = writeln!(out, "{json}");
                        let _ = out.flush();
                    }
                    last_assistant = Some(event);
                }
                "json" => {
                    // Accumulate for final result
                    let event = serde_json::json!({
                        "type": "assistant",
                        "message": {"content": clean, "role": "assistant"},
                    });
                    last_assistant = Some(event);
                }
                _ => {
                    // text mode: print directly
                    let _ = writeln!(out, "{clean}");
                    let _ = out.flush();
                    last_assistant = Some(serde_json::json!({
                        "type": "assistant",
                        "message": {"content": clean, "role": "assistant"},
                    }));
                }
            }
        }
    }

    // Wait for child
    let status = child.wait().ok();
    let exit_code = status.map(|s| s.code().unwrap_or(1)).unwrap_or(1);

    // Emit result
    emit_result(config, &last_assistant, &mut out);

    if config.emit_runtime_events {
        protocol::emit_stop_received(run_id, "");
    }

    // Footer (use kiro_session module for session ID)
    if config.show_session_footer {
        use crate::providers::kiro_session;
        let data_path = kiro_session::resolve_kiro_data_path();
        let before_ids = kiro_session::list_conversation_ids_for_cwd(&config.cwd, &data_path);
        // Re-query after spawn to find novel session
        if let Some(session_id) = kiro_session::resolve_session_id_after_spawn(
            &config.cwd,
            &before_ids,
            started_at_ms,
            &data_path,
        ) {
            interactive_providers::emit_session_footer(config.provider, &session_id);
        }
    }

    exit_code
}

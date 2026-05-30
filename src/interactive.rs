use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::child;
use crate::cleanup;
use crate::interactive_providers;
use crate::providers::ProviderKind;
use crate::sanitize;

const DEFAULT_IDLE_TIMEOUT_MS: u64 = 600_000;
const DEFAULT_HARD_TIMEOUT_MS: u64 = 3_600_000;
/// Quiescence window: how long PTY must be silent after session file growth stops.
const COMPLETION_QUIESCE_MS: u64 = 3_000;
/// How often to poll for completion.
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
        }
    }
}

pub fn run_interactive(config: InteractiveConfig) -> i32 {
    // Sanitize prompt
    let prompt = match sanitize::sanitize_prompt(&config.prompt) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("ai-e: prompt rejected: {e}");
            return 16;
        }
    };

    // Determine if prompt goes as positional arg or via paste
    let positional_prompt = if interactive_providers::accepts_positional_prompt(config.provider)
        && config.resume_session.is_none()
    {
        Some(prompt.as_str())
    } else {
        None
    };

    // Build TUI args
    let tui_args = interactive_providers::build_interactive_args(
        config.provider,
        positional_prompt,
        config.resume_session.as_deref(),
        config.model.as_deref(),
        &config.extra_args,
    );

    // Snapshot existing session files for set-diff detection
    let before_files = interactive_providers::list_session_files(config.provider, &config.cwd);
    let started_at_ms = epoch_ms().saturating_sub(1_000);

    // Set up signal handling
    let stop = Arc::new(AtomicBool::new(false));
    if let Err(e) =
        signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&stop))
    {
        log::warn!("SIGTERM handler registration failed: {e}");
    }
    if let Err(e) =
        signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&stop))
    {
        log::warn!("SIGINT handler registration failed: {e}");
    }

    // Spawn provider TUI in PTY
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
            eprintln!("ai-e: failed to spawn {} TUI: {e}", config.provider.id());
            return 4;
        }
    };

    let child_pid = pty_child.child.process_id().unwrap_or(0);
    eprintln!(
        "ai-e: {} interactive session started (pid {})",
        config.provider.id(),
        child_pid
    );

    // Wait for TUI to be ready (quiescence)
    pty_child.wait_quiescence(1500);

    // Inject prompt via bracketed paste if not passed as positional
    if positional_prompt.is_none() {
        let (paste_bytes, submit_bytes) = sanitize::bracketed_paste(&prompt);

        let inject_result = inject_prompt(
            &pty_child,
            &paste_bytes,
            &submit_bytes,
        );
        if let Err(e) = inject_result {
            eprintln!("ai-e: prompt injection failed: {e}");
            cleanup::kill_process_group(child_pid, "interactive", false);
            return 4;
        }
    }

    // Wait for completion: dual signal (PTY quiescence + session file growth stop)
    let last_activity = Arc::new(AtomicU64::new(epoch_ms()));
    let exit_code = wait_for_interactive_completion(
        &config,
        &mut pty_child,
        child_pid,
        &stop,
        &last_activity,
    );

    // Emit session footer
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
        let mut w = pty_child
            .writer
            .lock()
            .map_err(|_| "PTY writer lock poisoned".to_string())?;
        w.write_all(paste_bytes)
            .map_err(|e| format!("paste write failed: {e}"))?;
        let _ = w.flush();
    }
    std::thread::sleep(std::time::Duration::from_millis(150));
    {
        let mut w = pty_child
            .writer
            .lock()
            .map_err(|_| "PTY writer lock poisoned".to_string())?;
        w.write_all(submit_bytes)
            .map_err(|e| format!("submit write failed: {e}"))?;
        let _ = w.flush();
    }
    Ok(())
}

fn wait_for_interactive_completion(
    config: &InteractiveConfig,
    pty_child: &mut child::PtyChild,
    child_pid: u32,
    stop: &Arc<AtomicBool>,
    last_activity: &Arc<AtomicU64>,
) -> i32 {
    let start = std::time::Instant::now();
    let mut last_pty_change = epoch_ms();
    let mut session_file_len: u64 = 0;
    let mut session_file: Option<PathBuf> = None;
    let mut completion_candidate_since: Option<u64> = None;

    loop {
        // Check signals
        if stop.load(Ordering::Relaxed) {
            eprintln!("ai-e: interrupted");
            cleanup::graceful_exit(
                &pty_child.writer,
                &mut pty_child.child,
                child_pid,
                "interactive",
                false,
            );
            return 2;
        }

        // Check child exit
        if let Ok(Some(status)) = pty_child.child.try_wait() {
            return if status.success() { 0 } else { 1 };
        }

        // Track PTY activity via last_change_us
        let current_change = pty_child.last_change_us.load(Ordering::Relaxed);
        if current_change != last_pty_change {
            last_pty_change = current_change;
            last_activity.store(epoch_ms(), Ordering::Relaxed);
            completion_candidate_since = None;
        }

        // Try to find/track session file
        if session_file.is_none() {
            if let Some(dir) =
                interactive_providers::resolve_session_path(config.provider, &config.cwd)
            {
                if let Some(path) =
                    interactive_providers::find_newest_jsonl(&dir, started_at_ms_from(config))
                {
                    session_file_len = std::fs::metadata(&path)
                        .map(|m| m.len())
                        .unwrap_or(0);
                    session_file = Some(path);
                }
            }
        } else if let Some(ref path) = session_file {
            let new_len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            if new_len > session_file_len {
                session_file_len = new_len;
                last_activity.store(epoch_ms(), Ordering::Relaxed);
                completion_candidate_since = None;
            }
        }

        // Completion detection: PTY quiet + session file stable for COMPLETION_QUIESCE_MS
        let now = epoch_ms();
        let pty_idle_ms = now.saturating_sub(last_activity.load(Ordering::Relaxed));
        if pty_idle_ms >= COMPLETION_QUIESCE_MS && session_file.is_some() {
            match completion_candidate_since {
                None => {
                    completion_candidate_since = Some(now);
                }
                Some(since) if now.saturating_sub(since) >= COMPLETION_QUIESCE_MS => {
                    // Confirmed completion
                    eprintln!("ai-e: {} turn complete (quiescence)", config.provider.id());
                    send_exit_signal(pty_child, child_pid);
                    return 0;
                }
                _ => {}
            }
        }

        // Idle timeout
        let idle_elapsed = now.saturating_sub(last_activity.load(Ordering::Relaxed));
        if idle_elapsed > config.idle_timeout_ms {
            eprintln!(
                "ai-e: idle timeout: no activity for {}ms",
                config.idle_timeout_ms
            );
            cleanup::kill_process_group(child_pid, "interactive", false);
            return 6;
        }

        // Hard timeout
        if start.elapsed() > std::time::Duration::from_millis(config.hard_timeout_ms) {
            eprintln!(
                "ai-e: hard timeout: exceeded {}ms",
                config.hard_timeout_ms
            );
            cleanup::kill_process_group(child_pid, "interactive", false);
            return 6;
        }

        std::thread::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS));
    }
}

fn send_exit_signal(pty_child: &mut child::PtyChild, child_pid: u32) {
    // Try graceful: send Ctrl-C then wait briefly
    if let Ok(mut w) = pty_child.writer.lock() {
        let _ = w.write_all(b"\x03"); // Ctrl-C
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
    // Escalate
    cleanup::kill_process_group(child_pid, "interactive", false);
}

fn emit_footer(
    config: &InteractiveConfig,
    before_files: &std::collections::HashSet<PathBuf>,
    started_at_ms: u64,
) {
    // Find the new session file (set-diff)
    let after_files = interactive_providers::list_session_files(config.provider, &config.cwd);
    let new_files: Vec<_> = after_files.difference(before_files).collect();

    let session_id = if let Some(ref resume_id) = config.resume_session {
        Some(resume_id.clone())
    } else if let Some(new_file) = new_files.first() {
        interactive_providers::extract_session_id(config.provider, new_file)
    } else {
        // Fallback: find newest file after start
        interactive_providers::resolve_session_path(config.provider, &config.cwd)
            .and_then(|dir| interactive_providers::find_newest_jsonl(&dir, started_at_ms))
            .and_then(|path| interactive_providers::extract_session_id(config.provider, &path))
    };

    if let Some(id) = session_id {
        interactive_providers::emit_session_footer(config.provider, &id);
    }
}

fn started_at_ms_from(_config: &InteractiveConfig) -> u64 {
    // Approximate: current time minus hard timeout is too early, use 0 as floor
    // The actual started_at is passed through the completion loop implicitly
    0
}

fn epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

mod args;
mod child;
mod cleanup;
mod config;
mod headless;
mod hook;
mod normalize;
mod print_mode;
mod protocol;
mod providers;
mod sanitize;
mod terminal;
mod transcript;

use clap::Parser;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use args::{Cli, Command};
use config::RunConfig;
use providers::ProviderKind;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const PROMPT_ACCEPTANCE_TIMEOUT_MS: u64 = 8_000;

pub fn main_entry() {
    env_logger::init();
    let raw_args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let first = raw_args.first().and_then(|arg| arg.to_str());

    if matches!(first, Some("-V" | "--version" | "version")) {
        println!("ai-e {VERSION}");
        std::process::exit(0);
    }

    let (provider, raw_args) = providers::split_provider_args(raw_args);

    if is_top_level_help(&raw_args) {
        print_top_level_help();
        std::process::exit(0);
    }

    if provider.is_pty_provider() && !matches!(provider, ProviderKind::ClaudeCode) {
        let exit_code = headless::run_provider(provider, raw_args);
        std::process::exit(exit_code);
    }

    if !matches!(
        raw_args.first().and_then(|arg| arg.to_str()),
        Some("run" | "exec")
    ) {
        let stdin_prompt = match print_mode::read_stdin_if_piped() {
            Ok(input) => input,
            Err(e) => {
                eprintln!("ai-e: {e}");
                std::process::exit(16);
            }
        };
        let options = match print_mode::parse_print_mode_args(raw_args, stdin_prompt) {
            Ok(options) => options,
            Err(e) => {
                eprintln!("ai-e: {e}");
                std::process::exit(16);
            }
        };
        let prompt = options.prompt.clone();
        let config = print_mode::config_from_options(options);
        let exit_code = run_provider(provider, &config, Some(prompt));
        std::process::exit(exit_code);
    }

    let mut clap_args = vec![std::ffi::OsString::from("ai-e")];
    clap_args.extend(raw_args);
    let cli = Cli::parse_from(clap_args);

    match cli.command {
        Command::Run {
            jsonl: _,
            output_format,
            idle_timeout_ms,
            hard_timeout_ms,
            timeout_ms,
            claude_bin,
            cwd,
            cols,
            rows,
            resume,
            auto_accept_workspace_trust,
            terminal_tools,
            extra_args,
        } => {
            let effective_idle = if timeout_ms > 0 {
                timeout_ms
            } else {
                idle_timeout_ms
            };
            let config = RunConfig::new(
                claude_bin,
                cwd,
                cols,
                rows,
                effective_idle,
                hard_timeout_ms,
                output_format,
                resume,
                None,
                false,
                auto_accept_workspace_trust,
                extra_args,
                true,
                terminal_tools,
                false,
            );
            let exit_code = run_provider(provider, &config, None);
            std::process::exit(exit_code);
        }
    }
}

fn run_provider(
    provider: ProviderKind,
    config: &RunConfig,
    prompt_override: Option<String>,
) -> i32 {
    match provider {
        ProviderKind::ClaudeCode => run(config, prompt_override),
        ProviderKind::Codex | ProviderKind::Gemini | ProviderKind::Grok | ProviderKind::Copilot => {
            emit_error(
                config,
                &providers::unsupported_provider_message(provider),
                64,
            );
            64
        }
    }
}

fn is_top_level_help(args: &[std::ffi::OsString]) -> bool {
    let first = args.first().and_then(|arg| arg.to_str());
    let second = args.get(1).and_then(|arg| arg.to_str());

    matches!(first, None | Some("-h" | "--help" | "help"))
        || matches!(
            (first, second),
            (
                Some("p" | "print" | "-p" | "--print"),
                None | Some("-h" | "--help" | "help")
            )
        )
}

fn print_top_level_help() {
    println!(
        "\
ai-e {VERSION}

Modular PTY-backed executor for interactive AI CLIs

Usage:
  ai-e <provider> [options] <prompt>
  ai-e <provider> p [options] <prompt>
  ai-e <provider> run [wrapper flags] -- [provider args]

Providers:
  claude       Implemented. PTY-backed Claude Code execution.
  codex        PTY prompt-mode adapter using codex exec.
  gemini       PTY prompt-mode adapter using gemini --prompt.
  grok         PTY prompt-mode adapter using grok --single.
  copilot      PTY prompt-mode adapter using copilot --prompt.

Default provider:
  If the provider is omitted, ai-e currently defaults to claude for bootstrap
  compatibility. New cli-jaw integration should pass the provider explicitly.

Execution modes:
  claude uses the interactive PTY path copied from claude-e.
  codex/gemini/grok/copilot use native prompt-mode CLI surfaces inside PTY.

Options:
  -p, --print                              Print-compatible mode marker
  --input-format <text|stream-json>        Stdin input format
  --output-format <text|json|stream-json>   Output format for transcript replay
  --json-schema <schema>                   Append JSON-only schema instruction
  --model <model>                           Forward model to provider
  --effort <level>                          Forward effort to provider
  --permission-mode <mode>                  Forward permission mode to provider
  --allowed-tools <tools>                   Forward allowed tools to provider
  --disallowed-tools <tools>                Forward disallowed tools to provider
  --tools <tools>                           Forward tool set to provider
  --add-dir <directory>                     Forward additional working directory
  --mcp-config <config>                     Forward MCP config to provider
  --settings <file-or-json>                 Forward settings to provider
  --system-prompt <prompt>                  Forward system prompt to provider
  --append-system-prompt <prompt>           Forward appended system prompt
  --plugin-dir <path>                       Forward plugin directory to provider
  --plugin-url <url>                        Forward plugin URL to provider
  --session-id <uuid>                       Use session id for PTY run
  --no-session-persistence                  Suppress generated session id
  --verbose                                 Accepted print compatibility flag
  --include-partial-messages                Accepted print compatibility flag
  --include-hook-events                     Accepted print compatibility flag
  --replay-user-messages                    Accepted print compatibility flag
  --fallback-model <model>                  Accepted print compatibility flag
  --max-budget-usd <amount>                 Accepted print compatibility flag
  --claude-bin <path>                       Claude binary path for claude provider
  --cwd <path>                              Working directory
  --timeout-ms <ms>                         Runtime timeout
  --resume <session-id>                     Resume provider session when supported
  --auto-accept-workspace-trust             Accept workspace trust prompt
  --no-auto-accept-workspace-trust          Disable workspace trust auto-accept
  -t, --tool, --t                           Show compact tool progress on stderr
  --no-session-footer                       Hide final resume footer in print mode
  -h, --help                                Show this help
  -V, --version                             Show version

Examples:
  ai-e claude \"your prompt here\"
  ai-e claude p --model opus \"explain quicksort\"
  ai-e claude --tool \"use 10 tools and summarize the results\"
  ai-e claude --resume 00000000-0000-4000-8000-000000000001 \"continue\"
  ai-e claude --output-format stream-json \"audit src/\" --verbose
  ai-e claude --output-format json \"summarize this commit\" < commit.diff
  ai-e codex --model gpt-5-mini \"summarize this repo\"
  ai-e gemini --model gemini-2.5-pro \"summarize this repo\"
  ai-e grok --model auto \"summarize this repo\"
  ai-e copilot --model gpt-5-mini \"summarize this repo\"
"
    );
}

fn run(config: &RunConfig, prompt_override: Option<String>) -> i32 {
    emit_runtime_started(config);

    // Read prompt from stdin
    let prompt = if let Some(prompt) = prompt_override {
        prompt
    } else {
        match read_prompt() {
            Ok(p) => p,
            Err(e) => {
                emit_error(config, &e, 16);
                return 16;
            }
        }
    };

    // Sanitize prompt
    let prompt = match sanitize::sanitize_prompt(&prompt) {
        Ok(p) => p,
        Err(e) => {
            emit_error(config, &format!("prompt rejected: {e}"), 16);
            return 16;
        }
    };

    // Create hook directory with atomic sentinel relay
    let hook_dir = match hook::HookDir::create() {
        Ok(hd) => hd,
        Err(e) => {
            emit_error(config, &e, 13);
            return 13;
        }
    };

    // Build claude args
    let claude_args = build_claude_args(config, &hook_dir);

    // Set up signal handling
    let stop = Arc::new(AtomicBool::new(false));
    let stop_signal = Arc::clone(&stop);
    if let Err(e) =
        signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&stop_signal))
    {
        log::warn!("SIGTERM handler registration failed: {e}");
    }
    if let Err(e) =
        signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&stop_signal))
    {
        log::warn!("SIGINT handler registration failed: {e}");
    }

    // Spawn Claude in PTY
    let mut pty_child = match child::PtyChild::spawn(
        &config.claude_bin,
        &claude_args,
        &config.cwd,
        config.cols,
        config.rows,
        Arc::clone(&stop),
    ) {
        Ok(c) => c,
        Err(e) => {
            emit_error(config, &e, 4);
            return 4;
        }
    };

    let child_pid = pty_child.child.process_id().unwrap_or(0);
    emit_provider_spawned(config, child_pid);

    // Wait for SessionStart hook (also check for early child exit / resume failure)
    {
        let sentinel = hook_dir.sentinel_path("session-start");
        let start_wait = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(20_000);
        let mut trust_accept_attempted = false;
        loop {
            if sentinel.exists() {
                break;
            }
            if config.auto_accept_trust
                && !trust_accept_attempted
                && pty_child.try_auto_accept_workspace_trust()
            {
                trust_accept_attempted = true;
            }
            if let Ok(Some(status)) = pty_child.child.try_wait() {
                let code = if status.success() { 0 } else { 1 };
                emit_error(
                    config,
                    &format!("Claude exited before SessionStart (exit {})", code),
                    5,
                );
                return 5;
            }
            if start_wait.elapsed() > timeout {
                let screen = compact_screen_snapshot(&pty_child.screen_snapshot());
                let message = if screen.is_empty() {
                    "SessionStart timeout after 20s".to_string()
                } else {
                    format!("SessionStart timeout after 20s; screen: {screen}")
                };
                emit_error(config, &message, 5);
                cleanup::kill_process_group(child_pid, &config.run_id, config.emit_runtime_events);
                return 5;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    // Extract session info from SessionStart payload
    let session_payload = hook_dir.read_payload("session-start").unwrap_or_default();
    let transcript_path = hook::extract_transcript_path(&session_payload).unwrap_or_default();
    let session_id =
        hook::extract_session_id(&session_payload).unwrap_or_else(|| config.session_id.clone());

    emit_session_started(config, &session_id, &transcript_path);
    let transcript_path_buf = PathBuf::from(&transcript_path);
    let transcript_start_offset = if transcript_path.is_empty() {
        0
    } else {
        transcript::current_file_len(&transcript_path_buf).unwrap_or(0)
    };

    // Wait for PTY quiescence before injecting prompt
    pty_child.wait_quiescence(500);

    // Inject prompt via bracketed paste, then submit after a short delay
    let (paste_bytes, submit_bytes) = sanitize::bracketed_paste(&prompt);
    {
        let Ok(mut w) = pty_child.writer.lock() else {
            emit_error(config, "PTY writer lock poisoned before prompt write", 4);
            cleanup::kill_process_group(child_pid, &config.run_id, config.emit_runtime_events);
            return 4;
        };
        if let Err(e) = w.write_all(&paste_bytes) {
            emit_error(config, &format!("prompt write failed: {e}"), 4);
            cleanup::kill_process_group(child_pid, &config.run_id, config.emit_runtime_events);
            return 4;
        }
        let _ = w.flush();
    }
    // Brief delay so TUI processes the paste before receiving Enter
    std::thread::sleep(std::time::Duration::from_millis(150));
    {
        let Ok(mut w) = pty_child.writer.lock() else {
            emit_error(config, "PTY writer lock poisoned before submit write", 4);
            cleanup::kill_process_group(child_pid, &config.run_id, config.emit_runtime_events);
            return 4;
        };
        if let Err(e) = w.write_all(&submit_bytes) {
            emit_error(config, &format!("submit write failed: {e}"), 4);
            cleanup::kill_process_group(child_pid, &config.run_id, config.emit_runtime_events);
            return 4;
        }
        let _ = w.flush();
    }
    emit_prompt_injected(config);

    if transcript_path.is_empty() {
        emit_error(
            config,
            "prompt injection cannot be verified: SessionStart did not provide a transcript path",
            7,
        );
        cleanup::kill_process_group(child_pid, &config.run_id, config.emit_runtime_events);
        pty_child.join_drain();
        return 7;
    }

    match transcript::wait_for_prompt_activity_after_offset(
        &transcript_path_buf,
        transcript_start_offset,
        PROMPT_ACCEPTANCE_TIMEOUT_MS,
        stop.as_ref(),
    ) {
        Ok(true) => {}
        Ok(false) => {
            emit_error(
                config,
                &format!(
                    "prompt injection did not reach Claude transcript after {PROMPT_ACCEPTANCE_TIMEOUT_MS}ms"
                ),
                7,
            );
            cleanup::kill_process_group(child_pid, &config.run_id, config.emit_runtime_events);
            pty_child.join_drain();
            return 7;
        }
        Err(e) => {
            emit_error(
                config,
                &format!("prompt injection transcript verification failed: {e}"),
                7,
            );
            cleanup::kill_process_group(child_pid, &config.run_id, config.emit_runtime_events);
            pty_child.join_drain();
            return 7;
        }
    }

    // Shared activity tracker for idle timeout
    let last_activity = Arc::new(AtomicU64::new(epoch_ms()));
    let active_tools = Arc::new(AtomicUsize::new(0));

    // Start transcript tailing in a thread
    let transcript_stop = Arc::clone(&stop);
    let transcript_activity = Arc::clone(&last_activity);
    let transcript_active_tools = Arc::clone(&active_tools);
    let output_format = config.output_format.clone();
    let terminal_tools = config.terminal_tools;
    let transcript_handle = std::thread::spawn(move || {
        transcript::tail_transcript(
            &transcript_path_buf,
            transcript_stop,
            &output_format,
            transcript_start_offset,
            terminal_tools,
            Some(transcript_activity),
            Some(transcript_active_tools),
        )
    });

    // Wait for Stop/StopFailure or child exit
    let exit_code = wait_for_completion(
        config,
        &hook_dir,
        &mut pty_child,
        child_pid,
        &session_id,
        &last_activity,
        &active_tools,
    );

    // Signal transcript thread to finalize
    stop.store(true, Ordering::Relaxed);

    // Wait for transcript thread
    if let Ok(Ok(Some(last_assistant))) = transcript_handle.join() {
        if let Some(result_json) = normalize::synthesize_result(&last_assistant) {
            if config.output_format == "stream-json" || config.output_format == "json" {
                println!("{result_json}");
            }
        }
    }
    emit_session_footer(config, &session_id);

    // Cleanup
    pty_child.join_drain();

    // Brief delay before TempDir drop — ensures hook relay scripts finish writing
    std::thread::sleep(std::time::Duration::from_millis(200));
    drop(hook_dir);

    exit_code
}

fn wait_for_completion(
    config: &RunConfig,
    hook_dir: &hook::HookDir,
    pty_child: &mut child::PtyChild,
    child_pid: u32,
    session_id: &str,
    last_activity: &Arc<AtomicU64>,
    active_tools: &Arc<AtomicUsize>,
) -> i32 {
    let idle_timeout_ms = config.idle_timeout_ms;
    let hard_timeout_ms = config.hard_timeout_ms;
    let start = std::time::Instant::now();

    loop {
        // Check signals (SIGINT/SIGTERM → graceful exit, preserving session)
        if pty_child.stop.load(Ordering::Relaxed) {
            emit_interrupted(config, session_id);
            cleanup::graceful_exit(
                &pty_child.writer,
                &mut pty_child.child,
                child_pid,
                &config.run_id,
                config.emit_runtime_events,
            );
            return 2;
        }

        // Check Stop sentinel (normal completion)
        if hook_dir.sentinel_path("stop").exists() {
            let payload = hook_dir.read_payload("stop").unwrap_or_default();
            let tp = hook::extract_transcript_path(&payload).unwrap_or_default();
            emit_stop_received(config, &tp);

            wait_transcript_stable(&hook_dir.sentinel_path("stop"), 1000);

            cleanup::graceful_exit(
                &pty_child.writer,
                &mut pty_child.child,
                child_pid,
                &config.run_id,
                config.emit_runtime_events,
            );
            return 0;
        }

        // Check StopFailure sentinel
        if hook_dir.sentinel_path("stop-failure").exists() {
            let payload = hook_dir.read_payload("stop-failure").unwrap_or_default();
            let error = payload
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("unknown StopFailure");
            emit_stop_failure(config, error);
            cleanup::graceful_exit(
                &pty_child.writer,
                &mut pty_child.child,
                child_pid,
                &config.run_id,
                config.emit_runtime_events,
            );
            return 11;
        }

        // Check child exit — grace period to let sentinels finalize
        if let Ok(Some(status)) = pty_child.child.try_wait() {
            log::debug!("child exited with status: {:?}", status);
            std::thread::sleep(std::time::Duration::from_millis(300));

            // Re-check sentinels after child exit (Stop hook may have fired concurrently)
            if hook_dir.sentinel_path("stop").exists() {
                let payload = hook_dir.read_payload("stop").unwrap_or_default();
                let tp = hook::extract_transcript_path(&payload).unwrap_or_default();
                emit_stop_received(config, &tp);
                return 0;
            }
            if hook_dir.sentinel_path("stop-failure").exists() {
                let payload = hook_dir.read_payload("stop-failure").unwrap_or_default();
                let error = payload
                    .get("error")
                    .and_then(|e| e.as_str())
                    .unwrap_or("unknown StopFailure");
                emit_stop_failure(config, error);
                return 11;
            }

            return if status.success() { 0 } else { 1 };
        }

        // Idle timeout: no transcript activity for idle_timeout_ms (skipped while tool is active)
        let last_ts = last_activity.load(Ordering::Relaxed);
        let idle_elapsed = epoch_ms().saturating_sub(last_ts);
        if idle_elapsed > idle_timeout_ms && active_tools.load(Ordering::Relaxed) == 0 {
            emit_error(
                config,
                &format!("idle timeout: no transcript activity for {idle_timeout_ms}ms"),
                6,
            );
            cleanup::kill_process_group(child_pid, &config.run_id, config.emit_runtime_events);
            return 6;
        }

        // Hard timeout: absolute max runtime
        if start.elapsed() > std::time::Duration::from_millis(hard_timeout_ms) {
            emit_error(
                config,
                &format!("hard timeout: total runtime exceeded {hard_timeout_ms}ms"),
                6,
            );
            cleanup::kill_process_group(child_pid, &config.run_id, config.emit_runtime_events);
            return 6;
        }

        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

fn emit_runtime_started(config: &RunConfig) {
    if config.emit_runtime_events {
        protocol::emit_runtime_started(&config.run_id, VERSION);
    }
}

fn emit_provider_spawned(config: &RunConfig, child_pid: u32) {
    if config.emit_runtime_events {
        protocol::emit_provider_spawned(&config.run_id, "claude", child_pid);
    }
}

fn emit_session_started(config: &RunConfig, session_id: &str, transcript_path: &str) {
    if config.emit_runtime_events {
        protocol::emit_session_started(&config.run_id, session_id, transcript_path);
    }
}

fn emit_prompt_injected(config: &RunConfig) {
    if config.emit_runtime_events {
        protocol::emit_prompt_injected(&config.run_id);
    }
}

fn emit_stop_received(config: &RunConfig, transcript_path: &str) {
    if config.emit_runtime_events {
        protocol::emit_stop_received(&config.run_id, transcript_path);
    }
}

fn emit_stop_failure(config: &RunConfig, error: &str) {
    if config.emit_runtime_events {
        protocol::emit_stop_failure(&config.run_id, error);
    } else {
        eprintln!("ai-e: {error}");
    }
}

fn emit_interrupted(config: &RunConfig, session_id: &str) {
    if config.emit_runtime_events {
        protocol::emit_interrupted(&config.run_id, session_id);
    } else {
        eprintln!("ai-e: interrupted");
    }
}

fn emit_error(config: &RunConfig, message: &str, code: i32) {
    if config.emit_runtime_events {
        protocol::emit_error(&config.run_id, message, code);
    } else {
        eprintln!("ai-e: {message}");
    }
}

fn emit_session_footer(config: &RunConfig, session_id: &str) {
    if !config.show_session_footer || session_id.is_empty() {
        return;
    }
    eprintln!();
    eprintln!("[ai-e] session: {session_id}");
    eprintln!("[ai-e] resume: ai-e claude --resume {session_id} \"your next prompt\"");
}

fn wait_transcript_stable(sentinel_path: &std::path::Path, stable_ms: u64) {
    // Wait until sentinel file mtime is stable for stable_ms
    let start = std::time::Instant::now();
    let max_wait = std::time::Duration::from_millis(stable_ms * 3);

    while start.elapsed() < max_wait {
        if let Ok(meta) = std::fs::metadata(sentinel_path) {
            if let Ok(modified) = meta.modified() {
                let age = modified.elapsed().unwrap_or_default();
                if age >= std::time::Duration::from_millis(stable_ms) {
                    return;
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

fn compact_screen_snapshot(screen: &str) -> String {
    let compact = screen
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" | ");
    compact.chars().take(800).collect()
}

#[allow(clippy::cast_possible_truncation)]
fn epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

const MAX_PROMPT_BYTES: usize = 10 * 1024 * 1024; // 10MB

fn read_prompt() -> Result<String, String> {
    let mut prompt = String::new();
    std::io::stdin()
        .take((MAX_PROMPT_BYTES + 1) as u64)
        .read_to_string(&mut prompt)
        .map_err(|e| format!("stdin read failed: {e}"))?;

    if prompt.len() > MAX_PROMPT_BYTES {
        return Err(format!(
            "prompt too large ({} bytes, max {})",
            prompt.len(),
            MAX_PROMPT_BYTES
        ));
    }

    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return Err("prompt stdin is empty".to_string());
    }

    Ok(trimmed.to_string())
}

fn build_claude_args(config: &RunConfig, hook_dir: &hook::HookDir) -> Vec<String> {
    let mut args = Vec::new();

    if config.is_resume() {
        if let Some(ref session_id) = config.resume_session {
            args.push("--resume".to_string());
            args.push(session_id.clone());
        }
    } else if !config.no_session_persistence {
        args.push("--session-id".to_string());
        args.push(config.session_id.clone());
    }

    args.push("--settings".to_string());
    args.push(hook_dir.build_settings_json());

    args.extend(claude_args_with_permission_defaults(&config.extra_args));

    args
}

fn claude_args_with_permission_defaults(extra_args: &[String]) -> Vec<String> {
    let mut args = extra_args.to_vec();
    if !has_explicit_permission_policy(extra_args) {
        args.push("--dangerously-skip-permissions".to_string());
    }
    args
}

fn has_explicit_permission_policy(args: &[String]) -> bool {
    args.iter().any(|arg| {
        matches!(
            arg.as_str(),
            "--dangerously-skip-permissions"
                | "--allow-dangerously-skip-permissions"
                | "--permission-mode"
        ) || arg.starts_with("--permission-mode=")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn adds_permission_bypass_by_default() {
        assert_eq!(
            claude_args_with_permission_defaults(&strings(&["--model", "opus"])),
            strings(&["--model", "opus", "--dangerously-skip-permissions"])
        );
    }

    #[test]
    fn preserves_explicit_permission_mode() {
        assert_eq!(
            claude_args_with_permission_defaults(&strings(&[
                "--model",
                "opus",
                "--permission-mode",
                "auto"
            ])),
            strings(&["--model", "opus", "--permission-mode", "auto"])
        );
    }

    #[test]
    fn preserves_inline_explicit_permission_mode() {
        assert_eq!(
            claude_args_with_permission_defaults(&strings(&["--permission-mode=auto"])),
            strings(&["--permission-mode=auto"])
        );
    }

    #[test]
    fn does_not_duplicate_permission_bypass() {
        assert_eq!(
            claude_args_with_permission_defaults(&strings(&["--dangerously-skip-permissions"])),
            strings(&["--dangerously-skip-permissions"])
        );
    }

    #[test]
    fn idle_timeout_skipped_when_tool_active() {
        let last_activity = Arc::new(AtomicU64::new(epoch_ms() - 700_000));
        let active_tools = Arc::new(AtomicUsize::new(1));
        let idle_timeout_ms: u64 = 600_000;

        let last_ts = last_activity.load(Ordering::Relaxed);
        let idle_elapsed = epoch_ms().saturating_sub(last_ts);
        assert!(idle_elapsed > idle_timeout_ms);
        let should_kill =
            idle_elapsed > idle_timeout_ms && active_tools.load(Ordering::Relaxed) == 0;
        assert!(!should_kill);
    }

    #[test]
    fn idle_timeout_fires_when_no_tool_active() {
        let last_activity = Arc::new(AtomicU64::new(epoch_ms() - 700_000));
        let active_tools = Arc::new(AtomicUsize::new(0));
        let idle_timeout_ms: u64 = 600_000;

        let last_ts = last_activity.load(Ordering::Relaxed);
        let idle_elapsed = epoch_ms().saturating_sub(last_ts);
        let should_kill =
            idle_elapsed > idle_timeout_ms && active_tools.load(Ordering::Relaxed) == 0;
        assert!(should_kill);
    }

    #[test]
    fn idle_timeout_skipped_with_multiple_tools() {
        let last_activity = Arc::new(AtomicU64::new(epoch_ms() - 700_000));
        let active_tools = Arc::new(AtomicUsize::new(3));
        let idle_timeout_ms: u64 = 600_000;

        let last_ts = last_activity.load(Ordering::Relaxed);
        let idle_elapsed = epoch_ms().saturating_sub(last_ts);
        let should_kill =
            idle_elapsed > idle_timeout_ms && active_tools.load(Ordering::Relaxed) == 0;
        assert!(!should_kill);
    }
}

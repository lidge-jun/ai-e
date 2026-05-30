use std::collections::HashSet;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

use crate::print_mode;
use crate::providers::{ProviderKind, kiro_session};
use crate::sanitize;

const DEFAULT_TIMEOUT_MS: u64 = 600_000;

#[derive(Debug, PartialEq)]
pub struct HeadlessOptions {
    pub provider_bin: String,
    pub prompt: String,
    pub model: Option<String>,
    pub output_format: String,
    pub cwd: Option<PathBuf>,
    pub timeout_ms: u64,
    pub extra_args: Vec<String>,
    pub show_session_footer: bool,
}

pub fn run_provider(provider: ProviderKind, raw_args: Vec<OsString>) -> i32 {
    let stdin_prompt = match print_mode::read_stdin_if_piped() {
        Ok(input) => input,
        Err(e) => {
            eprintln!("ai-e: {e}");
            return 16;
        }
    };

    let options = match parse_headless_args(provider, raw_args, stdin_prompt) {
        Ok(options) => options,
        Err(e) => {
            eprintln!("ai-e: {e}");
            return 16;
        }
    };

    let prompt = match sanitize::sanitize_prompt(&options.prompt) {
        Ok(prompt) => prompt,
        Err(e) => {
            eprintln!("ai-e: prompt rejected: {e}");
            return 16;
        }
    };

    let provider_args = build_provider_args(provider, &options, &prompt);
    let capture = if matches!(provider, ProviderKind::Kiro) {
        Some(Arc::new(Mutex::new(Vec::new())))
    } else {
        None
    };

    let cwd = options.cwd.clone();
    let show_session_footer = options.show_session_footer;
    let resume_session_id = extract_resume_session_id(&options.extra_args);
    let before_ids = if matches!(provider, ProviderKind::Kiro) && resume_session_id.is_none() {
        cwd.as_deref()
            .map(|path| {
                kiro_session::list_conversation_ids_for_cwd(
                    path,
                    &kiro_session::resolve_kiro_data_path(),
                )
            })
            .unwrap_or_default()
    } else {
        HashSet::new()
    };
    let started_at_ms = epoch_ms().saturating_sub(1_000);

    let code = if matches!(provider, ProviderKind::Kiro) {
        spawn_pipe_with_timeout(
            provider,
            &options.provider_bin,
            &provider_args,
            cwd.as_deref(),
            options.timeout_ms,
            capture.clone(),
        )
    } else {
        spawn_pty_with_timeout(
            provider,
            &options.provider_bin,
            &provider_args,
            cwd.as_deref(),
            options.timeout_ms,
            capture.clone(),
        )
    };

    if matches!(provider, ProviderKind::Kiro) && show_session_footer {
        emit_kiro_session_footer(
            cwd.as_deref(),
            resume_session_id.as_deref(),
            &before_ids,
            started_at_ms,
            capture.as_ref(),
        );
    }

    code
}

pub fn parse_headless_args(
    provider: ProviderKind,
    raw_args: Vec<OsString>,
    stdin_prompt: Option<String>,
) -> Result<HeadlessOptions, String> {
    let args = raw_args
        .into_iter()
        .map(|arg| {
            arg.into_string()
                .map_err(|_| "non-UTF-8 arguments are not supported")
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut provider_bin = provider.resolve_binary();
    let mut model = None;
    let mut output_format = "text".to_string();
    let mut cwd = None;
    let mut timeout_ms = DEFAULT_TIMEOUT_MS;
    let mut extra_args = Vec::new();
    let mut prompt_parts = Vec::new();
    let mut show_session_footer = true;

    let mut index = 0;
    while index < args.len() {
        let raw = args[index].clone();
        let (flag, inline_value) = split_inline_option(&raw);

        match flag.as_str() {
            "run" | "exec" | "p" | "print" if index == 0 => {
                index += 1;
            }
            "-p" | "--print" => {
                index += 1;
            }
            "--provider-bin" => {
                provider_bin = take_value(&args, &mut index, "--provider-bin", inline_value)?;
            }
            "-m" | "--model" => {
                model = Some(take_value(&args, &mut index, &flag, inline_value)?);
            }
            "--output-format" => {
                output_format = take_value(&args, &mut index, "--output-format", inline_value)?;
                validate_output_format(provider, &output_format)?;
            }
            "--cwd" | "-C" | "--cd" => {
                cwd = Some(PathBuf::from(take_value(
                    &args,
                    &mut index,
                    &flag,
                    inline_value,
                )?));
            }
            "--timeout-ms" => {
                let raw = take_value(&args, &mut index, "--timeout-ms", inline_value)?;
                timeout_ms = raw
                    .parse::<u64>()
                    .map_err(|_| format!("invalid --timeout-ms value: {raw}"))?;
            }
            "--json-schema" => {
                let _ = take_value(&args, &mut index, "--json-schema", inline_value)?;
                return Err(format!(
                    "--json-schema is not yet supported for {} headless provider",
                    provider.id()
                ));
            }
            "--effort"
            | "--reasoning-effort"
            | "--permission-mode"
            | "--allowed-tools"
            | "--allowedTools"
            | "--disallowed-tools"
            | "--disallowedTools"
            | "--tools"
            | "--add-dir"
            | "--include-directories"
            | "--mcp-config"
            | "--settings"
            | "--system-prompt"
            | "--append-system-prompt"
            | "--plugin-dir"
            | "--plugin-url"
            | "--config"
            | "-c"
            | "--sandbox"
            | "-s"
            | "--profile"
            | "--ask-for-approval"
            | "--approval-mode"
            | "--allow-tool"
            | "--deny-tool"
            | "--allow-url"
            | "--deny-url"
            | "--available-tools"
            | "--excluded-tools"
            | "--secret-env-vars"
            | "--stream" => {
                let value = take_value(&args, &mut index, &flag, inline_value)?;
                extra_args.push(flag);
                extra_args.push(value);
            }
            "--session-id" | "--resume" | "-r" | "--input-format" | "--fallback-model"
            | "--max-budget-usd" => {
                let value = take_value(&args, &mut index, &flag, inline_value)?;
                extra_args.push(flag);
                extra_args.push(value);
            }
            "--tool"
            | "--t"
            | "-t"
            | "--verbose"
            | "--include-partial-messages"
            | "--include-hook-events"
            | "--replay-user-messages"
            | "--no-session-footer"
            | "--auto-accept-workspace-trust"
            | "--no-auto-accept-workspace-trust"
            | "--interactive"
            | "--headless" => {
                if flag == "--no-session-footer" {
                    show_session_footer = false;
                }
                index += 1;
            }
            "--" => {
                extra_args.extend(args.iter().skip(index + 1).cloned());
                break;
            }
            _ if raw.starts_with('-') => {
                extra_args.push(raw);
                index += 1;
            }
            _ => {
                prompt_parts.push(raw);
                index += 1;
            }
        }
    }

    let mut prompt = prompt_parts.join(" ");
    if let Some(stdin) = stdin_prompt {
        if prompt.trim().is_empty() {
            prompt = stdin;
        } else if !stdin.trim().is_empty() {
            prompt.push_str("\n\n<stdin>\n");
            prompt.push_str(stdin.trim());
            prompt.push_str("\n</stdin>");
        }
    }

    if prompt.trim().is_empty() {
        return Err(format!("{} prompt is empty", provider.id()));
    }

    Ok(HeadlessOptions {
        provider_bin,
        prompt,
        model,
        output_format,
        cwd,
        timeout_ms,
        extra_args,
        show_session_footer,
    })
}

pub fn build_provider_args(
    provider: ProviderKind,
    options: &HeadlessOptions,
    prompt: &str,
) -> Vec<String> {
    match provider {
        ProviderKind::ClaudeCode => unreachable!("Claude Code uses the PTY provider"),
        ProviderKind::Codex => build_codex_args(options, prompt),
        ProviderKind::Gemini => build_gemini_args(options, prompt),
        ProviderKind::Grok => build_grok_args(options, prompt),
        ProviderKind::Copilot => build_copilot_args(options, prompt),
        ProviderKind::Kiro => build_kiro_args(options, prompt),
    }
}

fn build_codex_args(options: &HeadlessOptions, prompt: &str) -> Vec<String> {
    let mut args = vec!["exec".to_string()];
    if let Some(model) = &options.model {
        args.extend(["--model".to_string(), model.clone()]);
    }
    if let Some(cwd) = &options.cwd {
        args.extend(["--cd".to_string(), cwd.display().to_string()]);
    }
    if options.output_format != "text" {
        args.push("--json".to_string());
    }
    if !contains_any(
        &options.extra_args,
        &["--dangerously-bypass-approvals-and-sandbox", "--sandbox"],
    ) {
        args.push("--dangerously-bypass-approvals-and-sandbox".to_string());
    }
    if !contains_any(&options.extra_args, &["--skip-git-repo-check"]) {
        args.push("--skip-git-repo-check".to_string());
    }
    args.extend(options.extra_args.clone());
    args.push(prompt.to_string());
    args
}

fn build_gemini_args(options: &HeadlessOptions, prompt: &str) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(model) = &options.model {
        args.extend(["--model".to_string(), model.clone()]);
    }
    args.extend(["--prompt".to_string(), prompt.to_string()]);
    args.extend(["--output-format".to_string(), options.output_format.clone()]);
    if !contains_any(&options.extra_args, &["--skip-trust"]) {
        args.push("--skip-trust".to_string());
    }
    if !contains_any(&options.extra_args, &["--approval-mode"]) {
        args.extend(["--approval-mode".to_string(), "yolo".to_string()]);
    }
    for dir in gemini_default_include_directories(&options.extra_args) {
        args.extend(["--include-directories".to_string(), dir]);
    }
    args.extend(options.extra_args.clone());
    args
}

fn build_grok_args(options: &HeadlessOptions, prompt: &str) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(model) = &options.model {
        args.extend(["--model".to_string(), model.clone()]);
    }
    let output_format = match options.output_format.as_str() {
        "stream-json" => "streaming-json",
        "json" => "json",
        _ => "plain",
    };
    args.extend([
        "--single".to_string(),
        prompt.to_string(),
        "--output-format".to_string(),
        output_format.to_string(),
    ]);
    if !contains_any(&options.extra_args, &["--no-alt-screen"]) {
        args.push("--no-alt-screen".to_string());
    }
    if !contains_any(&options.extra_args, &["--always-approve"]) {
        args.push("--always-approve".to_string());
    }
    if !contains_any(&options.extra_args, &["--permission-mode"]) {
        args.extend([
            "--permission-mode".to_string(),
            "bypassPermissions".to_string(),
        ]);
    }
    args.extend(options.extra_args.clone());
    args
}

fn build_copilot_args(options: &HeadlessOptions, prompt: &str) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(model) = &options.model {
        args.extend(["--model".to_string(), model.clone()]);
    }
    let output_format = if options.output_format == "text" {
        "text"
    } else {
        "json"
    };
    args.extend([
        "--prompt".to_string(),
        prompt.to_string(),
        "--output-format".to_string(),
        output_format.to_string(),
    ]);
    if !contains_any(&options.extra_args, &["--allow-all"]) {
        args.push("--allow-all".to_string());
    }
    if !contains_any(&options.extra_args, &["--stream"]) {
        args.extend(["--stream".to_string(), "off".to_string()]);
    }
    args.extend(options.extra_args.clone());
    args
}

fn build_kiro_args(options: &HeadlessOptions, prompt: &str) -> Vec<String> {
    let (filtered_extra, resume_id) = split_kiro_resume_args(&options.extra_args);
    let mut args = vec!["chat".to_string(), "--no-interactive".to_string()];
    if let Some(model) = &options.model {
        args.extend(["--model".to_string(), model.clone()]);
    }
    if let Some(session_id) = resume_id {
        args.extend(["--resume-id".to_string(), session_id]);
    }
    if !contains_any(&filtered_extra, &["--trust-all-tools", "--trust-tools"]) {
        args.push("--trust-all-tools".to_string());
    }
    args.extend(filtered_extra);
    args.push(prompt.to_string());
    args
}

fn split_kiro_resume_args(extra_args: &[String]) -> (Vec<String>, Option<String>) {
    let mut filtered = Vec::new();
    let mut resume_id = None;
    let mut index = 0;
    while index < extra_args.len() {
        match extra_args[index].as_str() {
            "--resume" | "--resume-id" => {
                resume_id = extra_args.get(index + 1).cloned();
                index += 2;
            }
            _ => {
                filtered.push(extra_args[index].clone());
                index += 1;
            }
        }
    }
    (filtered, resume_id)
}

fn extract_resume_session_id(extra_args: &[String]) -> Option<String> {
    split_kiro_resume_args(extra_args).1
}

fn emit_kiro_session_footer(
    cwd: Option<&std::path::Path>,
    resume_session_id: Option<&str>,
    before_ids: &HashSet<String>,
    started_at_ms: u64,
    capture: Option<&Arc<Mutex<Vec<u8>>>>,
) {
    if let Some(session_id) = resume_session_id.filter(|id| !id.trim().is_empty()) {
        kiro_session::emit_session_footer(session_id);
        return;
    }

    let captured = capture
        .and_then(|buf| buf.lock().ok())
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default();

    if let Some(session_id) = kiro_session::parse_session_id_from_stdout(&captured) {
        kiro_session::emit_session_footer(&session_id);
        return;
    }

    let Some(cwd) = cwd else {
        return;
    };

    let data_path = kiro_session::resolve_kiro_data_path();
    if let Some(session_id) =
        kiro_session::resolve_session_id_after_spawn(cwd, before_ids, started_at_ms, &data_path)
    {
        kiro_session::emit_session_footer(&session_id);
    }
}

fn spawn_pty_with_timeout(
    provider: ProviderKind,
    bin: &str,
    args: &[String],
    cwd: Option<&std::path::Path>,
    timeout_ms: u64,
    capture: Option<Arc<Mutex<Vec<u8>>>>,
) -> i32 {
    let pty_system = native_pty_system();
    let pair = match pty_system.openpty(PtySize {
        rows: 40,
        cols: 120,
        pixel_width: 0,
        pixel_height: 0,
    }) {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!(
                "ai-e: failed to open PTY for {} provider: {e}",
                provider.id()
            );
            return 4;
        }
    };

    let mut command = CommandBuilder::new(bin);
    command.args(args);
    if let Some(cwd) = cwd {
        command.cwd(cwd);
    }
    command.env("TERM", "xterm-256color");

    let mut child = match pair.slave.spawn_command(command) {
        Ok(child) => child,
        Err(e) => {
            eprintln!(
                "ai-e: failed to spawn {} provider in PTY via {bin}: {e}",
                provider.id()
            );
            return 4;
        }
    };

    drop(pair.slave);

    let mut reader = match pair.master.try_clone_reader() {
        Ok(reader) => reader,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            eprintln!(
                "ai-e: failed to open PTY reader for {} provider: {e}",
                provider.id()
            );
            return 4;
        }
    };
    drop(pair.master);

    let stop = Arc::new(AtomicBool::new(false));
    let last_activity_ms = Arc::new(AtomicU64::new(epoch_ms()));
    let reader_stop = Arc::clone(&stop);
    let reader_activity = Arc::clone(&last_activity_ms);
    let provider_id = provider.id().to_string();
    let reader_capture = capture.clone();
    let reader_handle = std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        let mut stdout = std::io::stdout().lock();
        let mut line_buf = Vec::<u8>::new();
        while !reader_stop.load(Ordering::Relaxed) {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    reader_activity.store(epoch_ms(), Ordering::Relaxed);
                    if let Some(capture_buf) = reader_capture.as_ref() {
                        if let Ok(mut guard) = capture_buf.lock() {
                            guard.extend_from_slice(&buf[..n]);
                        }
                    }
                    if let Err(e) =
                        write_projected_pty_chunk(provider, &buf[..n], &mut line_buf, &mut stdout)
                    {
                        eprintln!("ai-e: stdout write failed for {provider_id} provider: {e}");
                        break;
                    }
                }
                Err(e) => {
                    log::debug!("{} PTY read error: {e}", provider_id);
                    break;
                }
            }
        }
        let _ = flush_projected_pty_remainder(provider, &mut line_buf, &mut stdout);
    });

    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.exit_code() as i32,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    eprintln!(
                        "ai-e: {} provider timed out after {timeout_ms}ms",
                        provider.id()
                    );
                    break 6;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                eprintln!("ai-e: failed to wait for {} provider: {e}", provider.id());
                break 1;
            }
        }
    };

    stop.store(true, Ordering::Relaxed);
    let _ = reader_handle.join();
    code
}

/// Kiro `chat --no-interactive` hangs when wrapped in a PTY (detects TTY). Use pipes.
fn spawn_pipe_with_timeout(
    provider: ProviderKind,
    bin: &str,
    args: &[String],
    cwd: Option<&std::path::Path>,
    timeout_ms: u64,
    capture: Option<Arc<Mutex<Vec<u8>>>>,
) -> i32 {
    let mut command = std::process::Command::new(bin);
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command.stdin(std::process::Stdio::null());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            eprintln!(
                "ai-e: failed to spawn {} provider via {bin}: {e}",
                provider.id()
            );
            return 4;
        }
    };

    let stop = Arc::new(AtomicBool::new(false));
    let stdout_handle = child.stdout.take().map(|mut out| {
        let reader_stop = Arc::clone(&stop);
        let reader_capture = capture.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            let mut stdout = std::io::stdout().lock();
            while !reader_stop.load(Ordering::Relaxed) {
                match out.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if let Some(capture_buf) = reader_capture.as_ref() {
                            if let Ok(mut guard) = capture_buf.lock() {
                                guard.extend_from_slice(&buf[..n]);
                            }
                        }
                        let _ = stdout.write_all(&buf[..n]);
                        let _ = stdout.flush();
                    }
                    Err(e) => {
                        log::debug!("{} pipe stdout read error: {e}", provider.id());
                        break;
                    }
                }
            }
        })
    });
    let stderr_handle = child.stderr.take().map(|mut err| {
        let reader_stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            let mut stderr = std::io::stderr().lock();
            while !reader_stop.load(Ordering::Relaxed) {
                match err.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let _ = stderr.write_all(&buf[..n]);
                        let _ = stderr.flush();
                    }
                    Err(e) => {
                        log::debug!("{} pipe stderr read error: {e}", provider.id());
                        break;
                    }
                }
            }
        })
    });

    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code().unwrap_or(1),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    eprintln!(
                        "ai-e: {} provider timed out after {timeout_ms}ms",
                        provider.id()
                    );
                    break 6;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                eprintln!("ai-e: failed to wait for {} provider: {e}", provider.id());
                break 1;
            }
        }
    };

    stop.store(true, Ordering::Relaxed);
    if let Some(handle) = stdout_handle {
        let _ = handle.join();
    }
    if let Some(handle) = stderr_handle {
        let _ = handle.join();
    }
    code
}

fn validate_output_format(provider: ProviderKind, value: &str) -> Result<(), String> {
    let allowed = match provider {
        ProviderKind::Codex | ProviderKind::Copilot => &["text", "json", "stream-json"][..],
        ProviderKind::Gemini | ProviderKind::Grok | ProviderKind::Kiro => {
            &["text", "json", "stream-json"][..]
        }
        ProviderKind::ClaudeCode => unreachable!("Claude Code uses print_mode parser"),
    };
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "invalid --output-format for {}: {value}",
            provider.id()
        ))
    }
}

fn split_inline_option(raw: &str) -> (String, Option<String>) {
    if !raw.starts_with("--") {
        return (raw.to_string(), None);
    }
    raw.split_once('=')
        .map(|(flag, value)| (flag.to_string(), Some(value.to_string())))
        .unwrap_or_else(|| (raw.to_string(), None))
}

fn take_value(
    args: &[String],
    index: &mut usize,
    flag: &str,
    inline_value: Option<String>,
) -> Result<String, String> {
    if let Some(value) = inline_value {
        *index += 1;
        return Ok(value);
    }
    let value_index = *index + 1;
    let value = args
        .get(value_index)
        .ok_or_else(|| format!("missing value for {flag}"))?
        .clone();
    *index = value_index + 1;
    Ok(value)
}

fn contains_any(args: &[String], needles: &[&str]) -> bool {
    args.iter().any(|arg| {
        needles
            .iter()
            .any(|needle| arg == needle || arg.starts_with(&format!("{needle}=")))
    })
}

fn gemini_default_include_directories(extra_args: &[String]) -> Vec<String> {
    let mut dirs = Vec::new();
    for dir in [
        std::env::var("HOME").ok(),
        std::env::var("USERPROFILE").ok(),
    ]
    .into_iter()
    .flatten()
    {
        if dir.trim().is_empty()
            || dirs.contains(&dir)
            || extra_arg_has_value(extra_args, "--include-directories", &dir)
        {
            continue;
        }
        dirs.push(dir);
    }
    dirs
}

fn extra_arg_has_value(args: &[String], flag: &str, value: &str) -> bool {
    args.windows(2)
        .any(|pair| pair[0] == flag && pair[1] == value)
        || args.iter().any(|arg| arg == &format!("{flag}={value}"))
}

fn write_projected_pty_chunk<W: Write>(
    provider: ProviderKind,
    chunk: &[u8],
    line_buf: &mut Vec<u8>,
    stdout: &mut W,
) -> std::io::Result<()> {
    if !matches!(provider, ProviderKind::Copilot) {
        stdout.write_all(chunk)?;
        stdout.flush()?;
        return Ok(());
    }

    for &byte in chunk {
        if byte == b'\n' {
            write_projected_pty_line(provider, line_buf, stdout)?;
            line_buf.clear();
        } else {
            line_buf.push(byte);
        }
    }
    stdout.flush()
}

fn flush_projected_pty_remainder<W: Write>(
    provider: ProviderKind,
    line_buf: &mut Vec<u8>,
    stdout: &mut W,
) -> std::io::Result<()> {
    if line_buf.is_empty() {
        return Ok(());
    }
    if matches!(provider, ProviderKind::Copilot) {
        write_projected_pty_line(provider, line_buf, stdout)?;
        line_buf.clear();
    } else {
        stdout.write_all(line_buf)?;
        line_buf.clear();
    }
    stdout.flush()
}

fn write_projected_pty_line<W: Write>(
    provider: ProviderKind,
    line_buf: &[u8],
    stdout: &mut W,
) -> std::io::Result<()> {
    let line = std::str::from_utf8(line_buf).unwrap_or("");
    let projected = if matches!(provider, ProviderKind::Copilot) {
        project_copilot_jsonl_line(line)
    } else {
        None
    };
    match projected {
        Some(line) if line.is_empty() => Ok(()),
        Some(line) => writeln!(stdout, "{line}"),
        None => {
            stdout.write_all(line_buf)?;
            stdout.write_all(b"\n")
        }
    }
}

fn project_copilot_jsonl_line(line: &str) -> Option<String> {
    let trimmed = line.trim_end_matches('\r');
    let mut value = serde_json::from_str::<serde_json::Value>(trimmed).ok()?;
    if is_empty_copilot_reasoning_event(&value) {
        return Some(String::new());
    }
    strip_copilot_opaque_fields(&mut value);
    serde_json::to_string(&value).ok()
}

fn is_empty_copilot_reasoning_event(value: &serde_json::Value) -> bool {
    value
        .get("type")
        .and_then(|value| value.as_str())
        .is_some_and(|event_type| event_type == "assistant.reasoning")
        && value
            .pointer("/data/content")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .is_empty()
}

fn strip_copilot_opaque_fields(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            map.remove("reasoningOpaque");
            map.remove("encryptedContent");
            map.remove("reasoningId");
            for child in map.values_mut() {
                strip_copilot_opaque_fields(child);
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                strip_copilot_opaque_fields(child);
            }
        }
        _ => {}
    }
}

fn epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os_args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn parses_codex_gpt_5_mini_exec_shape() {
        let options = parse_headless_args(
            ProviderKind::Codex,
            os_args(&["--model", "gpt-5-mini", "--output-format", "json", "hello"]),
            None,
        )
        .unwrap();
        let args = build_provider_args(ProviderKind::Codex, &options, &options.prompt);
        assert_eq!(args[0], "exec");
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"gpt-5-mini".to_string()));
        assert!(args.contains(&"--json".to_string()));
        assert!(args.contains(&"--dangerously-bypass-approvals-and-sandbox".to_string()));
        assert!(args.contains(&"--skip-git-repo-check".to_string()));
    }

    #[test]
    fn builds_gemini_prompt_mode() {
        let options = parse_headless_args(
            ProviderKind::Gemini,
            os_args(&[
                "--model",
                "gemini-2.5-pro",
                "--output-format",
                "stream-json",
                "hello",
            ]),
            None,
        )
        .unwrap();
        let args = build_provider_args(ProviderKind::Gemini, &options, &options.prompt);
        assert_eq!(args[0], "--model");
        assert!(args.contains(&"gemini-2.5-pro".to_string()));
        assert!(args.contains(&"--prompt".to_string()));
        assert!(args.contains(&"hello".to_string()));
        assert!(args.contains(&"--output-format".to_string()));
        assert!(args.contains(&"stream-json".to_string()));
        assert!(args.contains(&"--skip-trust".to_string()));
        assert!(args.contains(&"--approval-mode".to_string()));
        assert!(args.contains(&"yolo".to_string()));
    }

    #[test]
    fn maps_grok_stream_json_name() {
        let options = parse_headless_args(
            ProviderKind::Grok,
            os_args(&["--output-format", "stream-json", "hello"]),
            None,
        )
        .unwrap();
        let args = build_provider_args(ProviderKind::Grok, &options, &options.prompt);
        assert!(args.contains(&"streaming-json".to_string()));
        assert!(args.contains(&"--no-alt-screen".to_string()));
        assert!(args.contains(&"--always-approve".to_string()));
        assert!(args.contains(&"--permission-mode".to_string()));
        assert!(args.contains(&"bypassPermissions".to_string()));
    }

    #[test]
    fn maps_copilot_stream_json_to_json() {
        let options = parse_headless_args(
            ProviderKind::Copilot,
            os_args(&[
                "--model",
                "gpt-5-mini",
                "--output-format",
                "stream-json",
                "hello",
            ]),
            None,
        )
        .unwrap();
        let args = build_provider_args(ProviderKind::Copilot, &options, &options.prompt);
        assert!(args.contains(&"--allow-all".to_string()));
        assert!(args.contains(&"--stream".to_string()));
        assert!(args.contains(&"off".to_string()));
        assert!(args.contains(&"json".to_string()));
        assert!(args.contains(&"gpt-5-mini".to_string()));
    }

    #[test]
    fn respects_explicit_headless_hardening_overrides() {
        let options = parse_headless_args(
            ProviderKind::Grok,
            os_args(&[
                "--output-format",
                "stream-json",
                "--permission-mode",
                "ask",
                "--always-approve",
                "hello",
            ]),
            None,
        )
        .unwrap();
        let args = build_provider_args(ProviderKind::Grok, &options, &options.prompt);
        assert_eq!(
            args.iter()
                .filter(|arg| arg.as_str() == "--permission-mode")
                .count(),
            1
        );
        assert!(!args.contains(&"bypassPermissions".to_string()));

        let options = parse_headless_args(
            ProviderKind::Copilot,
            os_args(&["--stream", "on", "hello"]),
            None,
        )
        .unwrap();
        let args = build_provider_args(ProviderKind::Copilot, &options, &options.prompt);
        assert_eq!(
            args.iter().filter(|arg| arg.as_str() == "--stream").count(),
            1
        );
        assert!(args.contains(&"on".to_string()));
    }

    #[test]
    fn builds_kiro_chat_no_interactive_shape() {
        let options = parse_headless_args(
            ProviderKind::Kiro,
            os_args(&["--model", "auto", "hello"]),
            None,
        )
        .unwrap();
        let args = build_provider_args(ProviderKind::Kiro, &options, &options.prompt);
        assert_eq!(args[0], "chat");
        assert_eq!(args[1], "--no-interactive");
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"auto".to_string()));
        assert!(args.contains(&"--trust-all-tools".to_string()));
        assert_eq!(args.last().map(String::as_str), Some("hello"));
    }

    #[test]
    fn maps_ai_e_resume_to_kiro_resume_id() {
        let options = parse_headless_args(
            ProviderKind::Kiro,
            os_args(&[
                "--resume",
                "79eee8a5-7c00-4cd9-8385-c534a2f8b814",
                "follow up",
            ]),
            None,
        )
        .unwrap();
        let args = build_provider_args(ProviderKind::Kiro, &options, &options.prompt);
        assert!(args.windows(2).any(|pair| {
            pair[0] == "--resume-id" && pair[1] == "79eee8a5-7c00-4cd9-8385-c534a2f8b814"
        }));
        assert!(!args.iter().any(|arg| arg == "--resume"));
        assert_eq!(args.last().map(String::as_str), Some("follow up"));
    }

    #[test]
    fn copilot_projection_strips_opaque_fields() {
        let line = r#"{"type":"assistant.message","data":{"content":"ok","reasoningOpaque":"secret","nested":{"encryptedContent":"secret"}}}"#;
        let projected = project_copilot_jsonl_line(line).unwrap();
        assert!(projected.contains(r#""content":"ok""#));
        assert!(!projected.contains("reasoningOpaque"));
        assert!(!projected.contains("encryptedContent"));
    }

    #[test]
    fn copilot_projection_drops_empty_reasoning_events() {
        let line = r#"{"type":"assistant.reasoning","data":{"content":"","reasoningId":"opaque"},"ephemeral":true}"#;
        assert_eq!(project_copilot_jsonl_line(line), Some(String::new()));
    }
}

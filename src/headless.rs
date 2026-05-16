use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::print_mode;
use crate::providers::ProviderKind;
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
    spawn_with_timeout(
        provider,
        &options.provider_bin,
        &provider_args,
        options.cwd.as_deref(),
        options.timeout_ms,
    )
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
            | "--secret-env-vars" => {
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
            | "--no-auto-accept-workspace-trust" => {
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
    args.extend([
        "--output-format".to_string(),
        options.output_format.clone(),
        "--skip-trust".to_string(),
        "--yolo".to_string(),
    ]);
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
        "--always-approve".to_string(),
        "--permission-mode".to_string(),
        "bypassPermissions".to_string(),
    ]);
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
        "--allow-all".to_string(),
        "--stream".to_string(),
        "off".to_string(),
    ]);
    args.extend(options.extra_args.clone());
    args
}

fn spawn_with_timeout(
    provider: ProviderKind,
    bin: &str,
    args: &[String],
    cwd: Option<&std::path::Path>,
    timeout_ms: u64,
) -> i32 {
    let mut command = Command::new(bin);
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command.stdin(Stdio::null());
    command.stdout(Stdio::inherit());
    command.stderr(Stdio::inherit());

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

    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.code().unwrap_or(1),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    eprintln!(
                        "ai-e: {} provider timed out after {timeout_ms}ms",
                        provider.id()
                    );
                    return 6;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                eprintln!("ai-e: failed to wait for {} provider: {e}", provider.id());
                return 1;
            }
        }
    }
}

fn validate_output_format(provider: ProviderKind, value: &str) -> Result<(), String> {
    let allowed = match provider {
        ProviderKind::Codex | ProviderKind::Copilot => &["text", "json", "stream-json"][..],
        ProviderKind::Gemini | ProviderKind::Grok => &["text", "json", "stream-json"][..],
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
        assert_eq!(
            args,
            vec![
                "--model",
                "gemini-2.5-pro",
                "--prompt",
                "hello",
                "--output-format",
                "stream-json",
                "--skip-trust",
                "--yolo",
            ]
        );
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
        assert!(args.contains(&"--always-approve".to_string()));
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
        assert!(args.contains(&"json".to_string()));
        assert!(args.contains(&"gpt-5-mini".to_string()));
    }
}

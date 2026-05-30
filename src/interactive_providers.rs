use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::providers::ProviderKind;

/// Build TUI launch args for interactive mode.
pub fn build_interactive_args(
    provider: ProviderKind,
    prompt: Option<&str>,
    resume_session: Option<&str>,
    model: Option<&str>,
    extra_args: &[String],
) -> Vec<String> {
    match provider {
        ProviderKind::Codex => build_codex_interactive(prompt, resume_session, model, extra_args),
        ProviderKind::Gemini => build_gemini_interactive(prompt, resume_session, model, extra_args),
        ProviderKind::Grok => build_grok_interactive(prompt, resume_session, model, extra_args),
        ProviderKind::Copilot => {
            build_copilot_interactive(prompt, resume_session, model, extra_args)
        }
        _ => Vec::new(),
    }
}

fn build_codex_interactive(
    prompt: Option<&str>,
    resume_session: Option<&str>,
    model: Option<&str>,
    extra_args: &[String],
) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(session_id) = resume_session {
        args.push("resume".to_string());
        args.push(session_id.to_string());
    }
    if let Some(m) = model {
        args.extend(["--model".to_string(), m.to_string()]);
    }
    args.push("--no-alt-screen".to_string());
    if !extra_args.iter().any(|a| a == "--dangerously-bypass-approvals-and-sandbox") {
        args.push("--dangerously-bypass-approvals-and-sandbox".to_string());
    }
    args.extend(extra_args.iter().cloned());
    if resume_session.is_none() {
        if let Some(p) = prompt {
            args.push(p.to_string());
        }
    }
    args
}

fn build_gemini_interactive(
    prompt: Option<&str>,
    resume_session: Option<&str>,
    model: Option<&str>,
    extra_args: &[String],
) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(m) = model {
        args.extend(["--model".to_string(), m.to_string()]);
    }
    if let Some(session_id) = resume_session {
        args.extend(["--resume".to_string(), session_id.to_string()]);
    }
    if !extra_args.iter().any(|a| a == "--skip-trust") {
        args.push("--skip-trust".to_string());
    }
    if !extra_args.iter().any(|a| a == "--approval-mode") {
        args.extend(["--approval-mode".to_string(), "yolo".to_string()]);
    }
    args.extend(extra_args.iter().cloned());
    if resume_session.is_none() {
        if let Some(p) = prompt {
            args.push(p.to_string());
        }
    }
    args
}

fn build_grok_interactive(
    prompt: Option<&str>,
    resume_session: Option<&str>,
    model: Option<&str>,
    extra_args: &[String],
) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(m) = model {
        args.extend(["--model".to_string(), m.to_string()]);
    }
    if let Some(session_id) = resume_session {
        args.extend(["--resume".to_string(), session_id.to_string()]);
    }
    args.push("--no-alt-screen".to_string());
    if !extra_args.iter().any(|a| a == "--always-approve") {
        args.push("--always-approve".to_string());
    }
    if !extra_args.iter().any(|a| a == "--permission-mode") {
        args.extend([
            "--permission-mode".to_string(),
            "bypassPermissions".to_string(),
        ]);
    }
    args.extend(extra_args.iter().cloned());
    // grok takes prompt via paste in interactive mode (no positional in TUI mode)
    // prompt will be injected via bracketed paste after TUI ready
    let _ = prompt;
    args
}

fn build_copilot_interactive(
    prompt: Option<&str>,
    resume_session: Option<&str>,
    model: Option<&str>,
    extra_args: &[String],
) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(m) = model {
        args.extend(["--model".to_string(), m.to_string()]);
    }
    if let Some(session_id) = resume_session {
        args.extend([format!("--resume={session_id}")]);
    }
    if !extra_args.iter().any(|a| a == "--allow-all" || a == "--yolo") {
        args.push("--yolo".to_string());
    }
    args.extend(extra_args.iter().cloned());
    // copilot takes prompt via paste in interactive mode
    let _ = prompt;
    args
}

/// Whether the provider accepts prompt as a positional arg in interactive TUI mode.
/// If false, prompt must be injected via bracketed paste after TUI is ready.
pub fn accepts_positional_prompt(provider: ProviderKind) -> bool {
    matches!(provider, ProviderKind::Codex | ProviderKind::Gemini)
}

/// Resolve the session file path to tail for completion detection.
/// Returns None if the path cannot be determined (provider doesn't write JSONL).
pub fn resolve_session_path(provider: ProviderKind, cwd: &Path) -> Option<PathBuf> {
    match provider {
        ProviderKind::Codex => resolve_codex_session_dir(),
        ProviderKind::Gemini => resolve_gemini_session_dir(cwd),
        ProviderKind::Grok => resolve_grok_session_dir(cwd),
        ProviderKind::Copilot => resolve_copilot_session_dir(),
        _ => None,
    }
}

fn resolve_codex_session_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let dir = PathBuf::from(home).join(".codex").join("sessions");
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

fn resolve_gemini_session_dir(cwd: &Path) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let gemini_tmp = PathBuf::from(home).join(".gemini").join("tmp");
    if !gemini_tmp.is_dir() {
        return None;
    }
    // Find the project-specific directory (gemini uses project-hash dirs)
    // Look for a dir that contains a "chats" subdirectory
    let entries = std::fs::read_dir(&gemini_tmp).ok()?;
    let cwd_str = cwd.to_string_lossy();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.join("chats").is_dir() {
            // Check if this project dir matches our cwd (heuristic: dir name contains cwd basename)
            let dir_name = path.file_name()?.to_string_lossy().to_string();
            let cwd_basename = cwd.file_name()?.to_string_lossy().to_string();
            if dir_name.contains(&cwd_basename) || cwd_str.contains(&dir_name) {
                return Some(path.join("chats"));
            }
        }
    }
    // Fallback: return the first project dir with chats
    let entries = std::fs::read_dir(&gemini_tmp).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.join("chats").is_dir() {
            return Some(path.join("chats"));
        }
    }
    None
}

fn resolve_grok_session_dir(cwd: &Path) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let sessions_dir = PathBuf::from(home).join(".grok").join("sessions");
    if !sessions_dir.is_dir() {
        return None;
    }
    // Grok URL-encodes the cwd as the directory name
    let encoded_cwd = urlencoded_path(cwd);
    let project_dir = sessions_dir.join(&encoded_cwd);
    if project_dir.is_dir() {
        Some(project_dir)
    } else {
        // Try to find a matching dir
        Some(sessions_dir)
    }
}

fn resolve_copilot_session_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let dir = PathBuf::from(home).join(".copilot").join("session-state");
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

/// Find the newest JSONL file in a directory (modified after `after_ms` epoch).
pub fn find_newest_jsonl(dir: &Path, after_ms: u64) -> Option<PathBuf> {
    find_newest_file_recursive(dir, "jsonl", after_ms)
}

fn find_newest_file_recursive(dir: &Path, ext: &str, after_ms: u64) -> Option<PathBuf> {
    let mut newest: Option<(PathBuf, u64)> = None;
    visit_dir_recursive(dir, ext, after_ms, &mut newest);
    newest.map(|(path, _)| path)
}

fn visit_dir_recursive(dir: &Path, ext: &str, after_ms: u64, newest: &mut Option<(PathBuf, u64)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit_dir_recursive(&path, ext, after_ms, newest);
        } else if path.extension().and_then(|e| e.to_str()) == Some(ext) {
            if let Some(mtime_ms) = file_mtime_ms(&path) {
                if mtime_ms >= after_ms {
                    if newest.as_ref().map_or(true, |(_, t)| mtime_ms > *t) {
                        *newest = Some((path, mtime_ms));
                    }
                }
            }
        }
    }
}

fn file_mtime_ms(path: &Path) -> Option<u64> {
    std::fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as u64)
}

/// Extract session ID from the session file path or content (provider-specific).
pub fn extract_session_id(provider: ProviderKind, session_path: &Path) -> Option<String> {
    match provider {
        ProviderKind::Codex => {
            // rollout-<timestamp>-<uuid>.jsonl → extract uuid
            let name = session_path.file_stem()?.to_string_lossy().to_string();
            // Format: rollout-1234567890-<uuid>
            let parts: Vec<&str> = name.splitn(3, '-').collect();
            if parts.len() >= 3 {
                Some(parts[2..].join("-"))
            } else {
                None
            }
        }
        ProviderKind::Gemini => {
            // session-<timestamp>-<id>.jsonl
            let name = session_path.file_stem()?.to_string_lossy().to_string();
            Some(name)
        }
        ProviderKind::Grok => {
            // Parent dir is the session UUID
            session_path.parent()?.file_name()?.to_str().map(String::from)
        }
        ProviderKind::Copilot => {
            // session-state/<uuid>/ → parent dir name
            session_path.parent()?.file_name()?.to_str().map(String::from)
        }
        _ => None,
    }
}

/// Emit the session footer for interactive mode.
pub fn emit_session_footer(provider: ProviderKind, session_id: &str) {
    eprintln!();
    eprintln!("[ai-e] session: {session_id}");
    eprintln!(
        "[ai-e] resume: ai-e {} --interactive --resume {session_id} \"your next prompt\"",
        provider.id()
    );
}

/// List existing session file paths before spawn (for set-diff detection).
pub fn list_session_files(provider: ProviderKind, cwd: &Path) -> HashSet<PathBuf> {
    let Some(dir) = resolve_session_path(provider, cwd) else {
        return HashSet::new();
    };
    collect_jsonl_files(&dir)
}

fn collect_jsonl_files(dir: &Path) -> HashSet<PathBuf> {
    let mut files = HashSet::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_jsonl_files(&path));
        } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            files.insert(path);
        }
    }
    files
}

fn urlencoded_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    s.replace('/', "%2F")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_interactive_args_with_prompt() {
        let args = build_interactive_args(
            ProviderKind::Codex,
            Some("hello"),
            None,
            Some("gpt-5-mini"),
            &[],
        );
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"gpt-5-mini".to_string()));
        assert!(args.contains(&"--no-alt-screen".to_string()));
        assert!(args.contains(&"--dangerously-bypass-approvals-and-sandbox".to_string()));
        assert_eq!(args.last().map(String::as_str), Some("hello"));
    }

    #[test]
    fn codex_interactive_args_resume() {
        let args = build_interactive_args(
            ProviderKind::Codex,
            Some("continue"),
            Some("abc-123"),
            None,
            &[],
        );
        assert_eq!(args[0], "resume");
        assert_eq!(args[1], "abc-123");
        // prompt not appended in resume mode (codex resume doesn't take positional)
        assert!(!args.contains(&"continue".to_string()));
    }

    #[test]
    fn gemini_interactive_args() {
        let args = build_interactive_args(
            ProviderKind::Gemini,
            Some("explain"),
            None,
            Some("gemini-2.5-pro"),
            &[],
        );
        assert!(args.contains(&"--skip-trust".to_string()));
        assert!(args.contains(&"--approval-mode".to_string()));
        assert!(args.contains(&"yolo".to_string()));
        assert_eq!(args.last().map(String::as_str), Some("explain"));
    }

    #[test]
    fn grok_interactive_no_positional_prompt() {
        let args = build_interactive_args(ProviderKind::Grok, Some("hello"), None, None, &[]);
        assert!(args.contains(&"--no-alt-screen".to_string()));
        assert!(args.contains(&"--always-approve".to_string()));
        // grok doesn't accept positional prompt in TUI mode
        assert!(!args.contains(&"hello".to_string()));
    }

    #[test]
    fn copilot_interactive_resume() {
        let args =
            build_interactive_args(ProviderKind::Copilot, None, Some("sess-uuid"), None, &[]);
        assert!(args.contains(&"--resume=sess-uuid".to_string()));
        assert!(args.contains(&"--yolo".to_string()));
    }

    #[test]
    fn urlencoded_path_encodes_slashes() {
        assert_eq!(
            urlencoded_path(Path::new("/Users/jun/project")),
            "%2FUsers%2Fjun%2Fproject"
        );
    }
}

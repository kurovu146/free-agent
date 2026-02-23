use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

/// Execute a bash command with timeout and output capture
pub async fn bash_exec(command: &str, working_dir: &str, timeout_secs: u64) -> String {
    if command.is_empty() {
        return "Error: empty command".into();
    }

    // Security: block dangerous patterns
    if is_dangerous_command(command) {
        return "Error: this command is blocked for safety. Dangerous operations like rm -rf /, format, or shutdown are not allowed.".into();
    }

    let dir = if working_dir.is_empty() { "." } else { working_dir };

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        Command::new("bash")
            .arg("-c")
            .arg(command)
            .current_dir(dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await;

    match result {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let exit_code = output.status.code().unwrap_or(-1);

            let mut result = String::new();
            if !stdout.is_empty() {
                result.push_str(&truncate_output(&stdout, 8000));
            }
            if !stderr.is_empty() {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str("[stderr]\n");
                result.push_str(&truncate_output(&stderr, 2000));
            }
            if exit_code != 0 {
                result.push_str(&format!("\n[exit code: {exit_code}]"));
            }
            if result.is_empty() {
                "(no output)".into()
            } else {
                result
            }
        }
        Ok(Err(e)) => format!("Failed to execute: {e}"),
        Err(_) => format!("Command timed out after {timeout_secs}s"),
    }
}

/// Read file contents with optional line range
pub async fn file_read(file_path: &str, offset: Option<usize>, limit: Option<usize>) -> String {
    if file_path.is_empty() {
        return "Error: empty file path".into();
    }

    let path = Path::new(file_path);
    if !path.exists() {
        return format!("Error: file not found: {file_path}");
    }
    if path.is_dir() {
        return format!("Error: {file_path} is a directory, not a file");
    }

    match tokio::fs::read_to_string(path).await {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let start = offset.unwrap_or(0);
            let count = limit.unwrap_or(2000);
            let end = (start + count).min(lines.len());

            if start >= lines.len() {
                return format!("Error: offset {start} exceeds file length ({} lines)", lines.len());
            }

            let selected: Vec<String> = lines[start..end]
                .iter()
                .enumerate()
                .map(|(i, line)| {
                    let line_num = start + i + 1;
                    let truncated = if line.len() > 2000 {
                        format!("{}...", &line[..2000])
                    } else {
                        line.to_string()
                    };
                    format!("{line_num:>6}\t{truncated}")
                })
                .collect();

            let mut result = selected.join("\n");
            if end < lines.len() {
                result.push_str(&format!("\n\n[... {} more lines]", lines.len() - end));
            }
            result
        }
        Err(e) => format!("Error reading file: {e}"),
    }
}

/// Write content to a file (create or overwrite)
pub async fn file_write(file_path: &str, content: &str) -> String {
    if file_path.is_empty() {
        return "Error: empty file path".into();
    }

    let path = Path::new(file_path);

    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                return format!("Error creating directories: {e}");
            }
        }
    }

    match tokio::fs::write(path, content).await {
        Ok(()) => {
            let lines = content.lines().count();
            let bytes = content.len();
            format!("Written {bytes} bytes ({lines} lines) to {file_path}")
        }
        Err(e) => format!("Error writing file: {e}"),
    }
}

/// Find files matching a glob pattern
pub async fn glob_search(pattern: &str, path: Option<&str>) -> String {
    if pattern.is_empty() {
        return "Error: empty pattern".into();
    }

    let base_dir = path.unwrap_or(".");

    // Extract filename part for -name matching
    let name_part = Path::new(pattern)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| pattern.to_string());

    let cmd = format!(
        "find {} -name '{}' -type f 2>/dev/null | head -50 | sort",
        shell_escape(base_dir),
        shell_escape(&name_part)
    );

    let output = Command::new("bash")
        .arg("-c")
        .arg(&cmd)
        .output()
        .await;

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.trim().is_empty() {
                format!("No files matching '{pattern}' in {base_dir}")
            } else {
                let files: Vec<&str> = stdout.trim().lines().collect();
                let count = files.len();
                let result = files.join("\n");
                if count >= 50 {
                    format!("{result}\n\n[showing first 50 results, there may be more]")
                } else {
                    format!("{result}\n\n[{count} files found]")
                }
            }
        }
        Err(e) => format!("Glob error: {e}"),
    }
}

/// Search file contents using grep/ripgrep
pub async fn grep_search(
    pattern: &str,
    path: Option<&str>,
    glob_filter: Option<&str>,
    case_insensitive: bool,
    context_lines: Option<u32>,
) -> String {
    if pattern.is_empty() {
        return "Error: empty search pattern".into();
    }

    let search_path = path.unwrap_or(".");

    // Prefer ripgrep if available, fallback to grep
    let (cmd_name, mut args) = if which_exists("rg").await {
        ("rg", vec![
            "--no-heading".to_string(),
            "--line-number".to_string(),
            "--max-count=100".to_string(),
            "--max-filesize=1M".to_string(),
        ])
    } else {
        ("grep", vec![
            "-rn".to_string(),
            "--max-count=100".to_string(),
        ])
    };

    if case_insensitive {
        args.push("-i".to_string());
    }

    if let Some(ctx) = context_lines {
        args.push(format!("-C{ctx}"));
    }

    if let Some(g) = glob_filter {
        if cmd_name == "rg" {
            args.push(format!("--glob={g}"));
        } else {
            args.push(format!("--include={g}"));
        }
    }

    args.push(pattern.to_string());
    args.push(search_path.to_string());

    let output = Command::new(cmd_name)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.trim().is_empty() {
                format!("No matches for '{pattern}' in {search_path}")
            } else {
                truncate_output(&stdout, 8000)
            }
        }
        Err(e) => format!("Grep error: {e}"),
    }
}

// --- Helpers ---

fn truncate_output(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!(
            "{}\n\n[... output truncated, {} chars total]",
            &text[..max_len],
            text.len()
        )
    }
}

fn is_dangerous_command(cmd: &str) -> bool {
    let lower = cmd.to_lowercase();
    let dangerous = [
        "rm -rf /",
        "rm -rf /*",
        "mkfs",
        "dd if=",
        ":(){:|:&};:",      // fork bomb
        "shutdown",
        "reboot",
        "init 0",
        "init 6",
        "halt",
        "poweroff",
        "> /dev/sda",
        "chmod -R 777 /",
    ];
    dangerous.iter().any(|d| lower.contains(d))
}

fn shell_escape(s: &str) -> String {
    // Simple escape: wrap in single quotes, escape existing single quotes
    format!("'{}'", s.replace('\'', "'\\''"))
}

async fn which_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

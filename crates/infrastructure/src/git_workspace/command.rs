use std::{
    collections::HashMap,
    path::Path,
    process::{Command, Output},
    thread,
    time::Duration,
};

use ai_chat_shared::{AppError, AppResult};

pub(super) fn run_git<I>(
    cwd: Option<&Path>,
    environment: &HashMap<String, String>,
    args: I,
) -> AppResult<String>
where
    I: IntoIterator<Item = String>,
{
    let args = args.into_iter().collect::<Vec<_>>();
    let max_attempts = if is_retryable_git_operation(&args) {
        3
    } else {
        1
    };
    for attempt in 0..max_attempts {
        let output = execute_git(cwd, environment, &args)?;
        if output.status.success() {
            return git_output(output);
        }
        let retryable = is_transient_git_network_error(&output.stderr);
        if !retryable || attempt + 1 == max_attempts {
            return git_output(output);
        }
        thread::sleep(Duration::from_millis(500 * (attempt as u64 + 1)));
    }
    unreachable!("Git retry loop always returns")
}

fn execute_git(
    cwd: Option<&Path>,
    environment: &HashMap<String, String>,
    args: &[String],
) -> AppResult<Output> {
    let mut command = Command::new("git");
    command.args(args).env("GIT_TERMINAL_PROMPT", "0");
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    for (key, value) in environment {
        command.env(key, value);
    }
    command.output().map_err(|error| {
        AppError::Validation(format!(
            "failed to start Git: {}",
            sanitize_error(&error.to_string())
        ))
    })
}

pub(super) fn is_retryable_git_operation(args: &[String]) -> bool {
    args.first().is_some_and(|operation| operation == "fetch")
}

pub(super) fn is_transient_git_network_error(stderr: &[u8]) -> bool {
    let stderr = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    [
        "connection reset by peer",
        "recv failure",
        "failed to connect",
        "could not resolve host",
        "remote end hung up unexpectedly",
        "gnutls recv error",
        "tls connection was non-properly terminated",
        "the requested url returned error: 502",
        "the requested url returned error: 503",
        "the requested url returned error: 504",
    ]
    .iter()
    .any(|pattern| stderr.contains(pattern))
}

pub(super) fn run_git_bytes<I>(
    cwd: Option<&Path>,
    environment: &HashMap<String, String>,
    args: I,
) -> AppResult<Vec<u8>>
where
    I: IntoIterator<Item = String>,
{
    let mut command = Command::new("git");
    command.args(args).env("GIT_TERMINAL_PROMPT", "0");
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    for (key, value) in environment {
        command.env(key, value);
    }
    let output = command.output().map_err(|error| {
        AppError::Validation(format!(
            "failed to start Git: {}",
            sanitize_error(&error.to_string())
        ))
    })?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    let stderr = sanitize_error(&String::from_utf8_lossy(&output.stderr));
    Err(AppError::Validation(format!(
        "Git command failed with status {}: {}",
        output.status,
        truncate(&stderr, 1_000)
    )))
}

fn git_output(output: Output) -> AppResult<String> {
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }
    let stderr = sanitize_error(&String::from_utf8_lossy(&output.stderr));
    Err(AppError::Validation(format!(
        "Git command failed with status {}: {}",
        output.status,
        truncate(&stderr, 1_000)
    )))
}

pub(super) fn ref_exists(
    mirror_path: &Path,
    environment: &HashMap<String, String>,
    reference: &str,
) -> AppResult<bool> {
    let mut command = Command::new("git");
    command
        .arg("--git-dir")
        .arg(mirror_path)
        .args(["show-ref", "--verify", "--quiet", reference])
        .env("GIT_TERMINAL_PROMPT", "0");
    for (key, value) in environment {
        command.env(key, value);
    }
    let status = command
        .status()
        .map_err(|error| AppError::Validation(format!("failed to inspect Git refs: {error}")))?;
    match status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(AppError::Validation(
            "Git ref inspection failed unexpectedly".into(),
        )),
    }
}

fn sanitize_error(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

fn truncate(value: &str, max_characters: usize) -> String {
    value.chars().take(max_characters).collect()
}

use super::*;
use std::{net::TcpListener, thread};

pub(super) fn managed_server(
    command: String,
    args: Vec<String>,
    startup_timeout_sec: u64,
) -> ManagedCodexMcpServer {
    let tool_approval_modes = ["navigate_page", "new_page", "upload_file"]
        .into_iter()
        .map(|tool| (tool.to_string(), "prompt".to_string()))
        .collect();
    ManagedCodexMcpServer {
        name: MANAGED_BROWSER_MCP_NAME.into(),
        command,
        args,
        env: BTreeMap::from([
            ("CHROME_DEVTOOLS_MCP_NO_UPDATE_CHECKS".into(), "1".into()),
            ("CHROME_DEVTOOLS_MCP_NO_USAGE_STATISTICS".into(), "1".into()),
        ]),
        url: None,
        env_http_headers: BTreeMap::new(),
        disabled_plugin_ids: vec![
            "browser@openai-bundled".into(),
            "chrome@openai-bundled".into(),
        ],
        required: false,
        startup_timeout_sec: Some(startup_timeout_sec),
        tool_timeout_sec: Some(180),
        default_tools_approval_mode: "approve".into(),
        tool_approval_modes,
        prompt_hint: None,
    }
}

pub(crate) fn host_mcp_args(command: &Path, endpoint: &str) -> Vec<String> {
    let mut args = Vec::new();
    if command
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.to_ascii_lowercase().starts_with("npx"))
    {
        args.extend(["--yes".into(), DEFAULT_BROWSER_MCP_PACKAGE.into()]);
    }
    args.extend([
        format!("--browserUrl={endpoint}"),
        "--experimentalPageIdRouting".into(),
        "--no-usage-statistics".into(),
        "--no-performance-crux".into(),
        "--allowUnrestrictedPaths".into(),
    ]);
    args
}

pub(super) fn parse_enabled(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "0" | "false" | "no" | "off"
    )
}

pub(super) fn configured_or_discovered_executable(
    variable: &str,
    candidates: &[&str],
) -> Option<PathBuf> {
    std::env::var_os(variable)
        .filter(|value| !value.is_empty())
        .and_then(|value| executable_in_path(&value.to_string_lossy()))
        .or_else(|| {
            candidates
                .iter()
                .find_map(|candidate| executable_in_path(candidate))
        })
}

pub(super) fn configured_or_discovered_browser() -> Option<PathBuf> {
    if let Some(configured) = std::env::var_os("RELAY_CHROME_EXECUTABLE")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    {
        return executable_in_path(&configured.to_string_lossy());
    }
    let fixed = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
        "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
        "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
    ];
    let fixed_browser = fixed
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.is_file());
    if fixed_browser.is_some() {
        return fixed_browser;
    }
    for root in ["LOCALAPPDATA", "PROGRAMFILES", "PROGRAMFILES(X86)"] {
        if let Some(path) = std::env::var_os(root)
            .map(PathBuf::from)
            .map(|path| path.join("Google/Chrome/Application/chrome.exe"))
            .filter(|path| path.is_file())
        {
            return Some(path);
        }
    }
    [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "chrome",
        "chrome.exe",
    ]
    .into_iter()
    .find_map(executable_in_path)
}

fn executable_in_path(name: &str) -> Option<PathBuf> {
    let candidate = Path::new(name);
    if candidate.components().count() > 1 {
        return candidate.is_file().then(|| candidate.to_path_buf());
    }
    let direct = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join(name))
        .find(|path| path.is_file());
    if direct.is_some() || !cfg!(windows) {
        return direct;
    }
    let extensions = std::env::var_os("PATHEXT")
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".into());
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .flat_map(|directory| {
            extensions
                .split(';')
                .map(move |extension| directory.join(format!("{name}{extension}")))
                .collect::<Vec<_>>()
        })
        .find(|path| path.is_file())
}

pub(super) fn reserve_loopback_port() -> AppResult<u16> {
    TcpListener::bind(("127.0.0.1", 0))
        .and_then(|listener| listener.local_addr())
        .map(|address| address.port())
        .map_err(|error| AppError::Internal(format!("cannot reserve browser port: {error}")))
}

pub(super) fn read_reusable_endpoint(profile: &Path) -> Option<String> {
    let endpoint = std::fs::read_to_string(profile.join(HOST_BROWSER_ENDPOINT_FILE)).ok()?;
    let endpoint = endpoint.trim().to_string();
    for _ in 0..3 {
        if browser_endpoint_ready(&endpoint) {
            return Some(endpoint);
        }
        thread::sleep(Duration::from_millis(100));
    }
    None
}

pub(super) fn create_browser_profile(path: &Path) -> AppResult<()> {
    std::fs::create_dir_all(path).map_err(|error| {
        AppError::Internal(format!("cannot create managed browser profile: {error}"))
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).map_err(
            |error| AppError::Internal(format!("cannot protect managed browser profile: {error}")),
        )?;
    }
    Ok(())
}

pub(super) fn create_browser_artifacts(workspace: &Path) -> AppResult<PathBuf> {
    let path = workspace.join(BROWSER_ARTIFACTS_RELATIVE_PATH);
    std::fs::create_dir_all(&path).map_err(|error| {
        AppError::Internal(format!(
            "cannot create managed browser artifact directory: {error}"
        ))
    })?;
    let path = path.canonicalize().map_err(|error| {
        AppError::Internal(format!(
            "cannot resolve managed browser artifact directory: {error}"
        ))
    })?;
    if !path.starts_with(workspace) {
        return Err(AppError::Validation(
            "managed browser artifact directory must remain inside the project workspace".into(),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).map_err(
            |error| {
                AppError::Internal(format!(
                    "cannot protect managed browser artifact directory: {error}"
                ))
            },
        )?;
    }
    Ok(path)
}

pub(super) fn remove_stale_chromium_runtime_files(profile: &Path) -> AppResult<()> {
    for name in CHROMIUM_RUNTIME_FILES {
        let path = profile.join(name);
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(AppError::Internal(format!(
                    "cannot remove stale Chromium runtime file {}: {error}",
                    path.display()
                )));
            }
        }
    }
    let _ = std::fs::remove_file(profile.join(HOST_BROWSER_ENDPOINT_FILE));
    Ok(())
}

#[cfg(unix)]
pub(super) fn browser_container_user(profile: &Path) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::metadata(profile).ok()?;
    Some(format!("{}:{}", metadata.uid(), metadata.gid()))
}

#[cfg(not(unix))]
pub(super) fn browser_container_user(_profile: &Path) -> Option<String> {
    None
}

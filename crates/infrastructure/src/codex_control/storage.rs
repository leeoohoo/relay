use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use ai_chat_shared::{AppError, AppResult};
use fs2::FileExt;
use serde::{de::DeserializeOwned, Serialize};
use uuid::Uuid;

use super::*;

impl CodexControlStore {
    pub(super) fn ensure_layout(&self) -> AppResult<()> {
        create_private_dir(&self.control_root)?;
        create_private_dir(&self.requests_dir())?;
        create_private_dir(&self.processing_dir())?;
        create_private_dir(&self.managed_cli_bin_dir())?;
        create_private_dir(&self.managed_cli_home())?;
        create_private_dir(&self.state_root.join("codex-profiles").join("homes"))?;
        Ok(())
    }

    pub(super) fn lock(&self) -> AppResult<ControlLock> {
        let open_lock = || {
            OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .truncate(false)
                .open(self.control_root.join(".lock"))
        };
        let file = match open_lock() {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.ensure_layout()?;
                open_lock().map_err(file_error)?
            }
            Err(error) => return Err(file_error(error)),
        };
        set_private_file_permissions(&file)?;
        file.lock_exclusive().map_err(file_error)?;
        Ok(ControlLock(file))
    }

    pub(super) fn runtime_path(&self) -> PathBuf {
        self.control_root.join("runtime.json")
    }

    pub(super) fn profiles_path(&self) -> PathBuf {
        self.control_root.join("profiles.json")
    }

    pub(super) fn mcp_catalog_path(&self) -> PathBuf {
        self.control_root.join("mcp-catalog.json")
    }

    pub(super) fn cli_settings_path(&self) -> PathBuf {
        self.control_root.join("cli-settings.json")
    }

    pub(super) fn trigger_preferences_path(&self) -> PathBuf {
        self.control_root.join("trigger-preferences.json")
    }

    pub(super) fn requests_dir(&self) -> PathBuf {
        self.control_root.join("requests")
    }

    pub(super) fn processing_dir(&self) -> PathBuf {
        self.control_root.join("processing")
    }

    pub(super) fn queued_request_path(&self, request_id: Uuid) -> PathBuf {
        self.requests_dir().join(format!("{request_id}.json"))
    }

    pub(super) fn read_runtime_unlocked(&self) -> AppResult<CodexCliRuntime> {
        read_json_or_default(&self.runtime_path())
    }

    pub(super) fn write_runtime_unlocked(&self, runtime: &CodexCliRuntime) -> AppResult<()> {
        write_private_json_atomic(&self.runtime_path(), runtime)
    }

    pub(super) fn read_profiles_unlocked(&self) -> AppResult<CodexAuthProfileFile> {
        read_json_or_default(&self.profiles_path())
    }

    pub(super) fn write_profiles_unlocked(&self, profiles: &CodexAuthProfileFile) -> AppResult<()> {
        write_private_json_atomic(&self.profiles_path(), profiles)
    }

    pub(super) fn read_mcp_catalog_unlocked(&self) -> AppResult<CodexMcpCatalogFile> {
        read_json_or_default(&self.mcp_catalog_path())
    }

    pub(super) fn write_mcp_catalog_unlocked(
        &self,
        catalog: &CodexMcpCatalogFile,
    ) -> AppResult<()> {
        write_private_json_atomic(&self.mcp_catalog_path(), catalog)
    }

    pub(super) fn read_cli_settings_unlocked(&self) -> AppResult<CompanyCodexCliSettingsFile> {
        read_json_or_default(&self.cli_settings_path())
    }

    pub(super) fn write_cli_settings_unlocked(
        &self,
        settings: &CompanyCodexCliSettingsFile,
    ) -> AppResult<()> {
        write_private_json_atomic(&self.cli_settings_path(), settings)
    }

    pub(super) fn read_trigger_preferences_unlocked(
        &self,
    ) -> AppResult<AgentTriggerPreferencesFile> {
        read_json_or_default(&self.trigger_preferences_path())
    }

    pub(super) fn write_trigger_preferences_unlocked(
        &self,
        preferences: &AgentTriggerPreferencesFile,
    ) -> AppResult<()> {
        write_private_json_atomic(&self.trigger_preferences_path(), preferences)
    }

    pub(super) fn write_request_unlocked(&self, request: &CodexControlRequest) -> AppResult<()> {
        write_private_json_atomic(&self.queued_request_path(request.id), request)
    }
}

pub(super) struct ControlLock(File);

impl Drop for ControlLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

pub(super) fn read_json_or_default<T: DeserializeOwned + Default>(path: &Path) -> AppResult<T> {
    if !path.exists() {
        return Ok(T::default());
    }
    read_json(path)
}

pub(super) fn read_json<T: DeserializeOwned>(path: &Path) -> AppResult<T> {
    let mut file = File::open(path).map_err(file_error)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(file_error)?;
    serde_json::from_slice(&bytes).map_err(|error| {
        AppError::Internal(format!(
            "cannot parse Codex control file {}: {error}",
            path.display()
        ))
    })
}

pub(super) fn write_private_json_atomic<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    create_private_dir(parent)?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("codex"),
        Uuid::new_v4()
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(file_error)?;
    set_private_file_permissions(&file)?;
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        AppError::Internal(format!("cannot serialize Codex control state: {error}"))
    })?;
    file.write_all(&bytes).map_err(file_error)?;
    file.sync_all().map_err(file_error)?;
    drop(file);
    fs::rename(&temporary, path).map_err(file_error)?;
    Ok(())
}

pub(super) fn create_private_dir(path: &Path) -> AppResult<()> {
    fs::create_dir_all(path).map_err(file_error)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let metadata = fs::metadata(path).map_err(file_error)?;
        if metadata.mode() & 0o777 != 0o700 {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(file_error)?;
        }
    }
    Ok(())
}

pub(super) fn set_private_file_permissions(file: &File) -> AppResult<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let metadata = file.metadata().map_err(file_error)?;
        if metadata.mode() & 0o777 != 0o600 {
            file.set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(file_error)?;
        }
    }
    Ok(())
}

pub(super) fn file_error(error: std::io::Error) -> AppError {
    AppError::Internal(format!("Codex control storage error: {error}"))
}

pub(super) fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

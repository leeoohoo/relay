use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use uuid::Uuid;

use ai_chat_shared::{AppError, AppResult};

#[derive(Clone)]
pub(super) struct HarnessCredentialStore {
    root: PathBuf,
}

impl HarnessCredentialStore {
    pub(super) fn at(root: PathBuf) -> AppResult<Self> {
        let root = if root.is_absolute() {
            root
        } else {
            std::env::current_dir()
                .map_err(credential_error)?
                .join(root)
        };
        ensure_private_directory(root.as_path())?;
        Ok(Self { root })
    }

    pub(super) fn password_path(&self, human_user_id: Uuid) -> PathBuf {
        self.root
            .join(format!("{}.password", human_user_id.simple()))
    }

    pub(super) fn token_path(&self, human_user_id: Uuid) -> PathBuf {
        self.root
            .join(format!("{}.access-token", human_user_id.simple()))
    }

    pub(super) fn read_password(&self, human_user_id: Uuid) -> AppResult<Option<String>> {
        read_secret(self.password_path(human_user_id).as_path())
    }

    pub(super) fn store_password(&self, human_user_id: Uuid, password: &str) -> AppResult<()> {
        atomic_write_secret(self.password_path(human_user_id).as_path(), password)
    }

    #[cfg(test)]
    pub(super) fn remove_password(&self, human_user_id: Uuid) -> AppResult<()> {
        remove_secret(self.password_path(human_user_id).as_path())
    }

    pub(super) fn store_access_token(&self, human_user_id: Uuid, token: &str) -> AppResult<()> {
        atomic_write_secret(self.token_path(human_user_id).as_path(), token)
    }

    pub(super) fn has_access_token(&self, human_user_id: Uuid) -> bool {
        self.token_path(human_user_id).is_file()
    }

    pub(super) fn read_access_token(&self, human_user_id: Uuid) -> AppResult<Option<String>> {
        read_secret(self.token_path(human_user_id).as_path())
    }

    pub(super) fn remove_access_token(&self, human_user_id: Uuid) -> AppResult<()> {
        remove_secret(self.token_path(human_user_id).as_path())
    }
}

fn ensure_private_directory(path: &Path) -> AppResult<()> {
    fs::create_dir_all(path).map_err(credential_error)?;
    set_mode(path, 0o700)
}

fn read_secret(path: &Path) -> AppResult<Option<String>> {
    match fs::read_to_string(path) {
        Ok(secret) => {
            let secret = secret.trim().to_string();
            if secret.is_empty() {
                Err(AppError::Internal(format!(
                    "Harness credential file is empty: {}",
                    path.display()
                )))
            } else {
                Ok(Some(secret))
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(credential_error(error)),
    }
}

fn atomic_write_secret(path: &Path, secret: &str) -> AppResult<()> {
    if secret.trim().is_empty() || secret.chars().count() > 8_192 {
        return Err(AppError::Validation(
            "Harness credential must contain 1 to 8192 characters".into(),
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| AppError::Internal("Harness credential path has no parent".into()))?;
    ensure_private_directory(parent)?;
    let temporary_path = parent.join(format!(".tmp-{}", Uuid::new_v4().simple()));
    let result = (|| -> AppResult<()> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary_path).map_err(credential_error)?;
        file.write_all(secret.as_bytes())
            .map_err(credential_error)?;
        file.sync_all().map_err(credential_error)?;
        set_mode(temporary_path.as_path(), 0o600)?;
        fs::rename(temporary_path.as_path(), path).map_err(credential_error)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary_path);
    }
    result
}

fn remove_secret(path: &Path) -> AppResult<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(credential_error(error)),
    }
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> AppResult<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(credential_error)
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> AppResult<()> {
    Ok(())
}

fn credential_error(error: std::io::Error) -> AppError {
    AppError::Internal(format!("Harness credential filesystem error: {error}"))
}

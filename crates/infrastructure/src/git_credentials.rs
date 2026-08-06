use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use uuid::Uuid;

use ai_chat_shared::{AppError, AppResult};

const GITHUB_TOKEN_PROFILE_PREFIX: &str = "github-token-";
const MANAGED_TOKEN_PROFILE_PREFIX: &str = "managed-git-token-";
const ASKPASS_SCRIPT: &str = r#"#!/bin/sh
case "$1" in
  *Username*) printf '%s\n' "x-access-token" ;;
  *Password*) exec cat "$RELAY_GITHUB_TOKEN_FILE" ;;
  *) exit 1 ;;
esac
"#;
const MANAGED_ASKPASS_SCRIPT: &str = r#"#!/bin/sh
case "$1" in
  *Username*) exec cat "$RELAY_GIT_USERNAME_FILE" ;;
  *Password*) exec cat "$RELAY_GIT_TOKEN_FILE" ;;
  *) exit 1 ;;
esac
"#;

#[derive(Debug, Clone)]
pub struct GitCredentialStore {
    root: PathBuf,
}

impl GitCredentialStore {
    pub fn from_env() -> AppResult<Self> {
        let root = std::env::var("AGENT_TRIGGER_GIT_CREDENTIALS_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".relay-agent-trigger/git-credentials"));
        Self::at(root)
    }

    pub fn at(root: PathBuf) -> AppResult<Self> {
        let root = absolute_path(root)?;
        ensure_private_directory(&root)?;
        let store = Self { root };
        store.ensure_askpass_script()?;
        store.ensure_managed_askpass_script()?;
        Ok(store)
    }

    pub fn store_github_token(&self, project_id: Uuid, token: &str) -> AppResult<()> {
        validate_github_token(token)?;
        ensure_private_directory(&self.root)?;
        atomic_write(&self.token_path(project_id), token.as_bytes(), 0o600)
    }

    pub fn remove_github_token(&self, project_id: Uuid) -> AppResult<()> {
        match fs::remove_file(self.token_path(project_id)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(credential_error(error)),
        }
    }

    pub fn has_github_token(&self, project_id: Uuid) -> bool {
        self.token_path(project_id).is_file()
    }

    pub fn store_managed_git_token(
        &self,
        project_id: Uuid,
        username: &str,
        token: &str,
    ) -> AppResult<String> {
        validate_git_username(username)?;
        validate_git_token(token)?;
        ensure_private_directory(&self.root)?;
        atomic_write(
            &self.managed_username_path(project_id),
            username.as_bytes(),
            0o600,
        )?;
        if let Err(error) = atomic_write(
            &self.managed_token_path(project_id),
            token.as_bytes(),
            0o600,
        ) {
            let _ = fs::remove_file(self.managed_username_path(project_id));
            return Err(error);
        }
        Ok(managed_token_profile_name(project_id))
    }

    pub fn has_managed_git_token(&self, project_id: Uuid) -> bool {
        self.managed_username_path(project_id).is_file()
            && self.managed_token_path(project_id).is_file()
    }

    pub fn remove_project_tokens(&self, project_id: Uuid) -> AppResult<()> {
        for path in [
            self.token_path(project_id),
            self.managed_username_path(project_id),
            self.managed_token_path(project_id),
        ] {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(credential_error(error)),
            }
        }
        Ok(())
    }

    pub fn auth_environment(&self, profile_name: &str) -> AppResult<HashMap<String, String>> {
        if is_managed_token_profile(profile_name) {
            let project_id =
                project_id_from_profile_with_prefix(profile_name, MANAGED_TOKEN_PROFILE_PREFIX)?;
            let username_path = self.managed_username_path(project_id);
            let token_path = self.managed_token_path(project_id);
            if !username_path.is_file() || !token_path.is_file() {
                return Err(AppError::Validation(
                    "managed Git credential is not configured on the Trigger host for this project"
                        .into(),
                ));
            }
            self.ensure_managed_askpass_script()?;
            return Ok(HashMap::from([
                (
                    "GIT_ASKPASS".into(),
                    path_string(&self.managed_askpass_path())?,
                ),
                (
                    "RELAY_GIT_USERNAME_FILE".into(),
                    path_string(&username_path)?,
                ),
                ("RELAY_GIT_TOKEN_FILE".into(), path_string(&token_path)?),
            ]));
        }
        let project_id = project_id_from_profile(profile_name)?;
        let token_path = self.token_path(project_id);
        if !token_path.is_file() {
            return Err(AppError::Validation(
                "GitHub Token is not configured on the Trigger host for this project".into(),
            ));
        }
        self.ensure_askpass_script()?;
        Ok(HashMap::from([
            ("GIT_ASKPASS".into(), path_string(&self.askpass_path())?),
            ("RELAY_GITHUB_TOKEN_FILE".into(), path_string(&token_path)?),
        ]))
    }

    fn token_path(&self, project_id: Uuid) -> PathBuf {
        self.root.join(format!("{}.token", project_id.simple()))
    }

    fn askpass_path(&self) -> PathBuf {
        self.root.join("github-askpass.sh")
    }

    fn managed_username_path(&self, project_id: Uuid) -> PathBuf {
        self.root.join(format!("{}.username", project_id.simple()))
    }

    fn managed_token_path(&self, project_id: Uuid) -> PathBuf {
        self.root
            .join(format!("{}.managed-token", project_id.simple()))
    }

    fn managed_askpass_path(&self) -> PathBuf {
        self.root.join("managed-askpass.sh")
    }

    fn ensure_askpass_script(&self) -> AppResult<()> {
        let path = self.askpass_path();
        if fs::read_to_string(&path).ok().as_deref() == Some(ASKPASS_SCRIPT) {
            set_mode(&path, 0o700)?;
            return Ok(());
        }
        atomic_write(&path, ASKPASS_SCRIPT.as_bytes(), 0o700)
    }

    fn ensure_managed_askpass_script(&self) -> AppResult<()> {
        let path = self.managed_askpass_path();
        if fs::read_to_string(&path).ok().as_deref() == Some(MANAGED_ASKPASS_SCRIPT) {
            set_mode(&path, 0o700)?;
            return Ok(());
        }
        atomic_write(&path, MANAGED_ASKPASS_SCRIPT.as_bytes(), 0o700)
    }
}

pub fn github_token_profile_name(project_id: Uuid) -> String {
    format!("{GITHUB_TOKEN_PROFILE_PREFIX}{}", project_id.simple())
}

pub fn is_github_token_profile(profile_name: &str) -> bool {
    profile_name.starts_with(GITHUB_TOKEN_PROFILE_PREFIX)
}

pub fn managed_token_profile_name(project_id: Uuid) -> String {
    format!("{MANAGED_TOKEN_PROFILE_PREFIX}{}", project_id.simple())
}

pub fn is_managed_token_profile(profile_name: &str) -> bool {
    profile_name.starts_with(MANAGED_TOKEN_PROFILE_PREFIX)
}

pub fn validate_github_token(token: &str) -> AppResult<()> {
    if !(20..=512).contains(&token.chars().count())
        || token
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err(AppError::Validation(
            "GitHub Token must contain 20 to 512 non-whitespace characters".into(),
        ));
    }
    Ok(())
}

fn project_id_from_profile(profile_name: &str) -> AppResult<Uuid> {
    project_id_from_profile_with_prefix(profile_name, GITHUB_TOKEN_PROFILE_PREFIX)
}

fn project_id_from_profile_with_prefix(profile_name: &str, prefix: &str) -> AppResult<Uuid> {
    let raw = profile_name
        .strip_prefix(prefix)
        .filter(|value| {
            value.len() == 32 && value.chars().all(|character| character.is_ascii_hexdigit())
        })
        .ok_or_else(|| AppError::Validation("invalid automatic GitHub Token profile".into()))?;
    Uuid::parse_str(raw)
        .map_err(|_| AppError::Validation("invalid automatic GitHub Token profile".into()))
}

fn validate_git_username(username: &str) -> AppResult<()> {
    if username.is_empty()
        || username != username.trim()
        || username.chars().count() > 256
        || username
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(AppError::Validation(
            "Git username must contain 1 to 256 non-whitespace characters".into(),
        ));
    }
    Ok(())
}

fn validate_git_token(token: &str) -> AppResult<()> {
    if !(20..=4096).contains(&token.chars().count())
        || token
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err(AppError::Validation(
            "Git token must contain 20 to 4096 non-whitespace characters".into(),
        ));
    }
    Ok(())
}

fn absolute_path(path: PathBuf) -> AppResult<PathBuf> {
    if path.is_absolute() {
        return Ok(path);
    }
    std::env::current_dir()
        .map(|current_dir| current_dir.join(path))
        .map_err(credential_error)
}

fn ensure_private_directory(path: &Path) -> AppResult<()> {
    fs::create_dir_all(path).map_err(credential_error)?;
    set_mode(path, 0o700)
}

fn atomic_write(path: &Path, contents: &[u8], mode: u32) -> AppResult<()> {
    let parent = path.parent().ok_or_else(|| {
        AppError::Validation("Git credential path has no parent directory".into())
    })?;
    ensure_private_directory(parent)?;
    let temporary_path = parent.join(format!(".tmp-{}", Uuid::new_v4().simple()));
    let result = (|| -> AppResult<()> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(mode);
        }
        let mut file = options.open(&temporary_path).map_err(credential_error)?;
        file.write_all(contents).map_err(credential_error)?;
        file.sync_all().map_err(credential_error)?;
        set_mode(&temporary_path, mode)?;
        fs::rename(&temporary_path, path).map_err(credential_error)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
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

fn path_string(path: &Path) -> AppResult<String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| AppError::Validation("Git credential path must be valid UTF-8".into()))
}

fn credential_error(error: std::io::Error) -> AppError {
    AppError::Validation(format!("Git credential filesystem error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> (PathBuf, GitCredentialStore) {
        let root =
            std::env::temp_dir().join(format!("relay-git-credentials-{}", Uuid::new_v4().simple()));
        let store = GitCredentialStore::at(root.clone()).expect("credential store");
        (root, store)
    }

    #[test]
    fn stores_overwrites_and_removes_github_tokens() {
        let (root, store) = test_store();
        let project_id = Uuid::new_v4();
        let first = "github_pat_first_token_1234567890";
        let second = "github_pat_second_token_1234567890";

        store
            .store_github_token(project_id, first)
            .expect("store token");
        assert!(store.has_github_token(project_id));
        assert_eq!(
            fs::read_to_string(store.token_path(project_id)).expect("read token"),
            first
        );

        store
            .store_github_token(project_id, second)
            .expect("overwrite token");
        assert_eq!(
            fs::read_to_string(store.token_path(project_id)).expect("read overwritten token"),
            second
        );

        store.remove_github_token(project_id).expect("remove token");
        assert!(!store.has_github_token(project_id));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn managed_project_credentials_use_private_askpass_files() {
        let (root, store) = test_store();
        let project_id = Uuid::new_v4();
        let profile = store
            .store_managed_git_token(project_id, "relay-bot", "managed_project_token_1234567890")
            .expect("store managed credential");
        assert_eq!(profile, managed_token_profile_name(project_id));
        assert!(store.has_managed_git_token(project_id));
        let environment = store
            .auth_environment(&profile)
            .expect("build managed credential environment");
        assert!(environment.contains_key("GIT_ASKPASS"));
        assert!(environment.contains_key("RELAY_GIT_USERNAME_FILE"));
        assert!(environment.contains_key("RELAY_GIT_TOKEN_FILE"));
        assert!(!environment
            .values()
            .any(|value| value.contains("managed_project_token")));
        store
            .remove_project_tokens(project_id)
            .expect("remove managed credential");
        assert!(!store.has_managed_git_token(project_id));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn automatic_profile_round_trips_without_exposing_token_in_environment() {
        let (root, store) = test_store();
        let project_id = Uuid::new_v4();
        let token = "github_pat_environment_token_1234567890";
        store
            .store_github_token(project_id, token)
            .expect("store token");

        let profile = github_token_profile_name(project_id);
        let environment = store.auth_environment(&profile).expect("environment");
        assert_eq!(
            project_id_from_profile(&profile).expect("project id"),
            project_id
        );
        assert!(environment.values().all(|value| !value.contains(token)));
        assert!(environment.contains_key("GIT_ASKPASS"));
        assert!(environment.contains_key("RELAY_GITHUB_TOKEN_FILE"));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn credential_files_have_private_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let (root, store) = test_store();
        let project_id = Uuid::new_v4();
        store
            .store_github_token(project_id, "github_pat_private_token_1234567890")
            .expect("store token");

        let root_mode = fs::metadata(&root)
            .expect("root metadata")
            .permissions()
            .mode()
            & 0o777;
        let token_mode = fs::metadata(store.token_path(project_id))
            .expect("token metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(root_mode, 0o700);
        assert_eq!(token_mode, 0o600);
        fs::remove_dir_all(root).expect("cleanup");
    }
}

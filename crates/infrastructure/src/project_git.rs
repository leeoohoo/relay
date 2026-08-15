use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use uuid::Uuid;

use ai_chat_shared::{AppError, AppResult};

use crate::git_credentials::GitCredentialStore;

#[derive(Debug, Clone)]
pub struct ProjectGitProvisionRequest {
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub project_name: String,
    pub description: String,
    pub initialize_default_branch: bool,
}

pub fn ensure_remote_default_branch(
    project_id: Uuid,
    remote_url: &str,
    default_branch: &str,
    auth_profile: &str,
    project_name: &str,
    description: &str,
    git_credentials: &GitCredentialStore,
) -> AppResult<bool> {
    let environment = git_credentials.auth_environment(auth_profile)?;
    if remote_branch_exists(remote_url, default_branch, &environment)? {
        return Ok(false);
    }

    let temporary = TemporaryGitDirectory::create(project_id)?;
    run_git(
        Some(temporary.path()),
        &environment,
        ["init", "-b", default_branch],
    )?;
    fs::write(
        temporary.path().join("README.md"),
        initial_readme(project_name, description),
    )
    .map_err(|error| AppError::Internal(format!("write initial project README: {error}")))?;
    run_git(Some(temporary.path()), &environment, ["add", "README.md"])?;
    run_git(
        Some(temporary.path()),
        &environment,
        [
            "-c",
            "user.name=Relay",
            "-c",
            "user.email=relay@localhost",
            "commit",
            "-m",
            "chore: initialize Relay project",
        ],
    )?;
    run_git(
        Some(temporary.path()),
        &environment,
        ["remote", "add", "origin", remote_url],
    )?;
    if let Err(error) = run_git(
        Some(temporary.path()),
        &environment,
        ["push", "-u", "origin", default_branch],
    ) {
        if remote_branch_exists(remote_url, default_branch, &environment)? {
            return Ok(false);
        }
        return Err(AppError::Validation(format!(
            "initialize project default branch {default_branch}: {error}"
        )));
    }
    Ok(true)
}

fn initial_readme(project_name: &str, description: &str) -> String {
    let name = project_name.trim();
    let description = description.trim();
    let mut content = format!(
        "# {}\n",
        if name.is_empty() {
            "Relay Project"
        } else {
            name
        }
    );
    if !description.is_empty() {
        content.push('\n');
        content.push_str(description);
        content.push('\n');
    }
    content.push_str("\nThis repository is managed by Relay.\n");
    content
}

fn remote_branch_exists(
    remote_url: &str,
    default_branch: &str,
    environment: &HashMap<String, String>,
) -> AppResult<bool> {
    let reference = format!("refs/heads/{default_branch}");
    let output = execute_git(
        None,
        environment,
        [
            "ls-remote",
            "--exit-code",
            "--heads",
            remote_url,
            reference.as_str(),
        ],
    )?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(2) => Ok(false),
        _ => Err(git_failure("inspect project default branch", output)),
    }
}

fn run_git<const N: usize>(
    cwd: Option<&Path>,
    environment: &HashMap<String, String>,
    args: [&str; N],
) -> AppResult<String> {
    let output = execute_git(cwd, environment, args)?;
    if !output.status.success() {
        return Err(git_failure("initialize project repository", output));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn execute_git<'a>(
    cwd: Option<&Path>,
    environment: &HashMap<String, String>,
    args: impl IntoIterator<Item = &'a str>,
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
            "failed to start Git while initializing project: {error}"
        ))
    })
}

fn git_failure(context: &str, output: Output) -> AppError {
    let stderr = String::from_utf8_lossy(&output.stderr).replace(['\r', '\n'], " ");
    AppError::Validation(format!(
        "{context} failed with status {}: {}",
        output.status,
        stderr.chars().take(1_000).collect::<String>()
    ))
}

struct TemporaryGitDirectory {
    path: PathBuf,
}

impl TemporaryGitDirectory {
    fn create(project_id: Uuid) -> AppResult<Self> {
        let path = std::env::temp_dir().join(format!(
            "relay-project-initialize-{}-{}",
            project_id.simple(),
            Uuid::new_v4().simple()
        ));
        fs::create_dir(&path).map_err(|error| {
            AppError::Internal(format!("create temporary project Git directory: {error}"))
        })?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryGitDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProvisionedProjectGit {
    pub remote_url: String,
    #[serde(skip_serializing)]
    pub push_url: Option<String>,
    pub default_branch: String,
    pub auth_profile: String,
    pub repository_identifier: String,
    #[serde(skip_serializing)]
    pub access_token_identifier: String,
}

pub trait ProjectGitProvisioner: Send + Sync {
    fn provision(&self, request: ProjectGitProvisionRequest) -> AppResult<ProvisionedProjectGit>;
}

pub fn generated_repository_identifier(project_name: &str, project_id: Uuid) -> String {
    let mut slug = project_name
        .chars()
        .flat_map(char::to_lowercase)
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    slug = slug.trim_matches('-').chars().take(36).collect();
    if slug.is_empty() {
        slug = "project".into();
    }
    format!("{}-{}", slug, &project_id.simple().to_string()[..8])
}

pub fn initial_project_access_token_identifier(project_id: Uuid) -> String {
    format!("relay-project-{}", &project_id.simple().to_string()[..12])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initializes_an_empty_remote_once() {
        let root = std::env::temp_dir().join(format!(
            "relay-project-git-test-{}",
            Uuid::new_v4().simple()
        ));
        let remote = root.join("remote.git");
        let credentials_root = root.join("credentials");
        fs::create_dir_all(&root).unwrap();
        let output = Command::new("git")
            .args(["init", "--bare", remote.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(output.status.success());
        let project_id = Uuid::new_v4();
        let credentials = GitCredentialStore::at(credentials_root).unwrap();
        let auth_profile = credentials
            .store_managed_git_token(project_id, "relay", "test-token-with-enough-length")
            .unwrap();

        assert!(ensure_remote_default_branch(
            project_id,
            remote.to_str().unwrap(),
            "main",
            &auth_profile,
            "Novel Project",
            "Initial story planning",
            &credentials,
        )
        .unwrap());
        assert!(!ensure_remote_default_branch(
            project_id,
            remote.to_str().unwrap(),
            "main",
            &auth_profile,
            "Novel Project",
            "Initial story planning",
            &credentials,
        )
        .unwrap());

        let reference = Command::new("git")
            .args([
                "--git-dir",
                remote.to_str().unwrap(),
                "show-ref",
                "--verify",
                "refs/heads/main",
            ])
            .output()
            .unwrap();
        assert!(reference.status.success());
        let readme = Command::new("git")
            .args([
                "--git-dir",
                remote.to_str().unwrap(),
                "show",
                "main:README.md",
            ])
            .output()
            .unwrap();
        assert!(readme.status.success());
        let readme = String::from_utf8(readme.stdout).unwrap();
        assert!(readme.contains("# Novel Project"));
        assert!(readme.contains("Initial story planning"));

        let _ = fs::remove_dir_all(root);
    }
}

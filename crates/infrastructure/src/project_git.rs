use uuid::Uuid;

use ai_chat_shared::AppResult;

#[derive(Debug, Clone)]
pub struct ProjectGitProvisionRequest {
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub project_name: String,
    pub description: String,
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

use base64::Engine as _;
use reqwest::{Method, Url};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use ai_chat_application::PlatformRepository;
use ai_chat_shared::{AppError, AppResult};

use super::{HarnessProvisioner, HarnessRequestError};

const MAX_HARNESS_FILE_BYTES: usize = 1024 * 1024;
const HARNESS_BRANCH_PAGE_SIZE: usize = 100;
const MAX_HARNESS_BRANCH_PAGES: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessRepositoryRef {
    pub name: String,
    pub commit: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessRepositoryEntry {
    pub name: String,
    pub path: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessRepositoryFile {
    pub name: String,
    pub path: String,
    pub size: usize,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarnessRepositoryContent {
    Directory {
        path: String,
        entries: Vec<HarnessRepositoryEntry>,
    },
    File(HarnessRepositoryFile),
    Unsupported {
        path: String,
        kind: String,
    },
}

#[derive(Debug, Deserialize)]
struct HarnessBranchResponse {
    name: String,
    sha: String,
    #[serde(default)]
    is_default: bool,
}

#[derive(Debug, Deserialize)]
struct HarnessContentResponse {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    content: Value,
}

#[derive(Debug, Deserialize)]
struct HarnessDirectoryContent {
    #[serde(default)]
    entries: Vec<HarnessDirectoryEntry>,
}

#[derive(Debug, Deserialize)]
struct HarnessDirectoryEntry {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    path: String,
}

#[derive(Debug, Deserialize)]
struct HarnessFileContent {
    #[serde(default)]
    encoding: String,
    #[serde(default)]
    data: String,
    #[serde(default)]
    size: usize,
    #[serde(default)]
    data_size: usize,
}

struct HarnessRepositoryAccess {
    api_base_url: String,
    repository_path: String,
    access_token: String,
}

impl<R: PlatformRepository> HarnessProvisioner<R> {
    pub fn is_managed_repository(&self, human_user_id: Uuid, remote_url: &str) -> AppResult<bool> {
        Ok(self.repository_access(human_user_id, remote_url)?.is_some())
    }

    pub async fn list_repository_refs(
        &self,
        human_user_id: Uuid,
        remote_url: &str,
    ) -> AppResult<Option<Vec<HarnessRepositoryRef>>> {
        let Some(access) = self.repository_access(human_user_id, remote_url)? else {
            return Ok(None);
        };
        let mut branches = Vec::new();
        for page_number in 1..=MAX_HARNESS_BRANCH_PAGES {
            let mut endpoint = harness_repository_endpoint(
                access.api_base_url.as_str(),
                access.repository_path.as_str(),
                &["branches"],
            )?;
            let limit = HARNESS_BRANCH_PAGE_SIZE.to_string();
            let page = page_number.to_string();
            endpoint.query_pairs_mut().extend_pairs([
                ("limit", limit.as_str()),
                ("page", page.as_str()),
                ("include_commit", "false"),
                ("sort", "name"),
                ("order", "asc"),
            ]);
            let page_branches = self
                .request_json::<Vec<HarnessBranchResponse>, ()>(
                    Method::GET,
                    endpoint.as_str(),
                    Some(access.access_token.as_str()),
                    None,
                )
                .await
                .map_err(harness_repository_error)?;
            let page_len = page_branches.len();
            branches.extend(page_branches);
            if page_len < HARNESS_BRANCH_PAGE_SIZE {
                break;
            }
            if page_number == MAX_HARNESS_BRANCH_PAGES {
                return Err(AppError::Validation(format!(
                    "Harness repository branch count exceeds {}",
                    HARNESS_BRANCH_PAGE_SIZE * MAX_HARNESS_BRANCH_PAGES
                )));
            }
        }
        Ok(Some(
            branches
                .into_iter()
                .filter(|branch| !branch.name.trim().is_empty() && !branch.sha.trim().is_empty())
                .map(|branch| HarnessRepositoryRef {
                    name: branch.name,
                    commit: branch.sha,
                    is_default: branch.is_default,
                })
                .collect(),
        ))
    }

    pub async fn get_repository_content(
        &self,
        human_user_id: Uuid,
        remote_url: &str,
        reference: &str,
        path: &str,
    ) -> AppResult<Option<HarnessRepositoryContent>> {
        let Some(access) = self.repository_access(human_user_id, remote_url)? else {
            return Ok(None);
        };
        let path_parts = path
            .split('/')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        let mut endpoint_parts = Vec::with_capacity(path_parts.len() + 1);
        endpoint_parts.push("content");
        endpoint_parts.extend(path_parts);
        let mut endpoint = harness_repository_endpoint(
            access.api_base_url.as_str(),
            access.repository_path.as_str(),
            endpoint_parts.as_slice(),
        )?;
        if path.is_empty() {
            let root_path = format!("{}/", endpoint.path().trim_end_matches('/'));
            endpoint.set_path(root_path.as_str());
        }
        endpoint
            .query_pairs_mut()
            .append_pair("git_ref", reference)
            .append_pair("include_commit", "false")
            .append_pair("flatten_directories", "false");
        let content = self
            .request_json::<HarnessContentResponse, ()>(
                Method::GET,
                endpoint.as_str(),
                Some(access.access_token.as_str()),
                None,
            )
            .await
            .map_err(harness_repository_error)?;
        Ok(Some(parse_harness_content(content)?))
    }

    fn repository_access(
        &self,
        human_user_id: Uuid,
        remote_url: &str,
    ) -> AppResult<Option<HarnessRepositoryAccess>> {
        if !self.is_enabled() {
            return Ok(None);
        }
        let Some(account) = self
            .repo
            .get_human_harness_account_result(human_user_id)?
            .filter(|account| {
                account.status == ai_chat_domain::agent_identity::HUMAN_HARNESS_STATUS_ACTIVE
            })
        else {
            return Ok(None);
        };
        let Some(api_base_url) = self.config.api_base_url.clone() else {
            return Ok(None);
        };
        let remote = Url::parse(remote_url)
            .map_err(|error| AppError::Validation(format!("invalid project Git URL: {error}")))?;
        let public_base = Url::parse(account.harness_base_url.as_str())
            .map_err(|error| AppError::Internal(format!("invalid stored Harness URL: {error}")))?;
        if remote.scheme() != public_base.scheme()
            || remote.host_str() != public_base.host_str()
            || remote.port_or_known_default() != public_base.port_or_known_default()
        {
            return Ok(None);
        }
        let repository_path = remote
            .path()
            .trim_matches('/')
            .strip_prefix("git/")
            .and_then(|path| path.strip_suffix(".git"))
            .filter(|path| {
                path.strip_prefix(account.space_identifier.as_str())
                    .is_some_and(|suffix| suffix.starts_with('/') && suffix.len() > 1)
            })
            .map(ToOwned::to_owned);
        let Some(repository_path) = repository_path else {
            return Ok(None);
        };
        let access_token = self
            .credentials
            .read_access_token(human_user_id)?
            .ok_or_else(|| AppError::Validation("Harness access token is unavailable".into()))?;
        Ok(Some(HarnessRepositoryAccess {
            api_base_url,
            repository_path,
            access_token,
        }))
    }
}

fn harness_repository_endpoint(
    base_url: &str,
    repository_path: &str,
    operation_parts: &[&str],
) -> AppResult<Url> {
    let mut endpoint = Url::parse(base_url)
        .map_err(|error| AppError::Validation(format!("invalid Harness API URL: {error}")))?;
    {
        let mut segments = endpoint.path_segments_mut().map_err(|_| {
            AppError::Validation("Harness API URL cannot contain an opaque path".into())
        })?;
        segments.pop_if_empty();
        segments.extend(["api", "v1", "repos"]);
        segments.extend(repository_path.split('/').filter(|part| !part.is_empty()));
        segments.push("+");
        segments.extend(operation_parts.iter().copied());
    }
    Ok(endpoint)
}

fn parse_harness_content(content: HarnessContentResponse) -> AppResult<HarnessRepositoryContent> {
    match content.kind.as_str() {
        "dir" => {
            let directory = serde_json::from_value::<HarnessDirectoryContent>(content.content)
                .map_err(|error| {
                    AppError::Internal(format!("parse Harness directory response: {error}"))
                })?;
            Ok(HarnessRepositoryContent::Directory {
                path: content.path,
                entries: directory
                    .entries
                    .into_iter()
                    .map(|entry| HarnessRepositoryEntry {
                        name: entry.name,
                        path: entry.path,
                        kind: match entry.kind.as_str() {
                            "dir" => "directory".into(),
                            other => other.to_string(),
                        },
                    })
                    .collect(),
            })
        }
        "file" => {
            let file =
                serde_json::from_value::<HarnessFileContent>(content.content).map_err(|error| {
                    AppError::Internal(format!("parse Harness file response: {error}"))
                })?;
            let bytes = if file.encoding.eq_ignore_ascii_case("base64") {
                base64::engine::general_purpose::STANDARD
                    .decode(file.data.as_bytes())
                    .map_err(|error| {
                        AppError::Internal(format!("decode Harness file response: {error}"))
                    })?
            } else {
                file.data.into_bytes()
            };
            let size = file.size.max(bytes.len());
            if size > MAX_HARNESS_FILE_BYTES || file.data_size > MAX_HARNESS_FILE_BYTES {
                return Err(AppError::Validation(format!(
                    "repository file is larger than the {} MiB preview limit",
                    MAX_HARNESS_FILE_BYTES / (1024 * 1024)
                )));
            }
            Ok(HarnessRepositoryContent::File(HarnessRepositoryFile {
                name: if content.name.is_empty() {
                    content.path.rsplit('/').next().unwrap_or_default().into()
                } else {
                    content.name
                },
                path: content.path,
                size,
                bytes,
            }))
        }
        other => Ok(HarnessRepositoryContent::Unsupported {
            path: content.path,
            kind: other.to_string(),
        }),
    }
}

fn harness_repository_error(error: HarnessRequestError) -> AppError {
    AppError::Validation(format!("Harness repository request failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_harness_repository_endpoints_without_checkout_paths() {
        let endpoint = harness_repository_endpoint(
            "https://harness.example.test/base",
            "space/repository",
            &["content", "src", "main.rs"],
        )
        .expect("endpoint");

        assert_eq!(
            endpoint.as_str(),
            "https://harness.example.test/base/api/v1/repos/space/repository/+/content/src/main.rs"
        );
    }

    #[test]
    fn parses_base64_harness_files() {
        let content = HarnessContentResponse {
            kind: "file".into(),
            name: "main.rs".into(),
            path: "src/main.rs".into(),
            content: serde_json::json!({
                "encoding": "base64",
                "data": "Zm4gbWFpbigpIHt9Cg==",
                "size": 13,
                "data_size": 13
            }),
        };

        let HarnessRepositoryContent::File(file) = parse_harness_content(content).expect("file")
        else {
            panic!("expected file");
        };
        assert_eq!(file.bytes, b"fn main() {}\n");
    }
}

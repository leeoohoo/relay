use std::collections::HashMap;
use std::io::Read;
use std::time::Duration;

use reqwest::{blocking::Client, redirect::Policy, Url};
use serde_json::{json, Value};

use ai_chat_application::{
    AgentRuntimeModelPlan, AgentRuntimeModelProvider, AgentRuntimeModelRequest,
    AgentRuntimeModelResponse,
};
use ai_chat_domain::company::AGENT_RUNTIME_MODEL_PROVIDER_OPENAI_RESPONSES;
use ai_chat_shared::{AppError, AppResult};

use crate::config::RuntimeSecretResolverConfig;

const MAX_SECRET_CONFIG_BYTES: usize = 256 * 1024;
const MAX_VAULT_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_PROVIDER_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone)]
struct VaultSecretResolver {
    base_url: Url,
    token: String,
    namespace: Option<String>,
    timeout_seconds: u64,
}

impl std::fmt::Debug for VaultSecretResolver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VaultSecretResolver")
            .field("base_url", &self.base_url)
            .field("token", &"[REDACTED]")
            .field("namespace", &self.namespace)
            .field("timeout_seconds", &self.timeout_seconds)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
struct RuntimeSecretResolver {
    secrets: HashMap<String, String>,
    vault: Option<VaultSecretResolver>,
}

impl std::fmt::Debug for RuntimeSecretResolver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeSecretResolver")
            .field("configured_secret_count", &self.secrets.len())
            .field("vault", &self.vault)
            .finish()
    }
}

impl RuntimeSecretResolver {
    fn from_config(config: RuntimeSecretResolverConfig) -> anyhow::Result<Self> {
        let secrets = Self::parse_secrets_json(config.secrets_json.as_deref())?;
        let vault = match (config.vault_addr, config.vault_token) {
            (None, None) => None,
            (Some(_), None) => {
                anyhow::bail!(
                    "AGENT_RUNTIME_VAULT_ADDR requires AGENT_RUNTIME_VAULT_TOKEN or VAULT_TOKEN"
                )
            }
            (None, Some(_)) => {
                anyhow::bail!(
                    "AGENT_RUNTIME_VAULT_TOKEN or VAULT_TOKEN requires AGENT_RUNTIME_VAULT_ADDR"
                )
            }
            (Some(address), Some(token)) => Some(Self::build_vault_resolver(
                &address,
                token,
                config.vault_namespace,
                config.allow_insecure_vault_http,
                config.vault_timeout_seconds,
            )?),
        };
        Ok(Self { secrets, vault })
    }

    fn parse_secrets_json(raw: Option<&str>) -> anyhow::Result<HashMap<String, String>> {
        let Some(raw) = raw else {
            return Ok(HashMap::new());
        };
        if raw.len() > MAX_SECRET_CONFIG_BYTES {
            anyhow::bail!("AGENT_RUNTIME_SECRETS_JSON exceeds the configured size limit");
        }
        let value: Value = serde_json::from_str(raw)
            .map_err(|_| anyhow::anyhow!("AGENT_RUNTIME_SECRETS_JSON must be a JSON object"))?;
        let object = value
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("AGENT_RUNTIME_SECRETS_JSON must be a JSON object"))?;
        if object.len() > 256 {
            anyhow::bail!("AGENT_RUNTIME_SECRETS_JSON contains too many entries");
        }
        let mut secrets = HashMap::with_capacity(object.len());
        for (name, value) in object {
            Self::validate_secret_name(name).map_err(|_| {
                anyhow::anyhow!("AGENT_RUNTIME_SECRETS_JSON contains an invalid key")
            })?;
            let secret = value.as_str().ok_or_else(|| {
                anyhow::anyhow!("AGENT_RUNTIME_SECRETS_JSON values must be strings")
            })?;
            if secret.is_empty() {
                anyhow::bail!("AGENT_RUNTIME_SECRETS_JSON values cannot be empty");
            }
            secrets.insert(name.clone(), secret.to_string());
        }
        Ok(secrets)
    }

    fn build_vault_resolver(
        address: &str,
        token: String,
        namespace: Option<String>,
        allow_insecure_http: bool,
        timeout_seconds: u64,
    ) -> anyhow::Result<VaultSecretResolver> {
        let mut base_url = Url::parse(address.trim())?;
        if base_url.scheme() != "https" && !(allow_insecure_http && base_url.scheme() == "http") {
            anyhow::bail!(
                "AGENT_RUNTIME_VAULT_ADDR must use https unless insecure Vault HTTP is explicitly enabled"
            );
        }
        if !base_url.username().is_empty() || base_url.password().is_some() {
            anyhow::bail!("AGENT_RUNTIME_VAULT_ADDR cannot contain credentials");
        }
        if base_url.query().is_some() || base_url.fragment().is_some() {
            anyhow::bail!("AGENT_RUNTIME_VAULT_ADDR cannot contain a query or fragment");
        }
        if !matches!(base_url.path(), "" | "/") {
            anyhow::bail!("AGENT_RUNTIME_VAULT_ADDR cannot contain a path");
        }
        if token.is_empty() {
            anyhow::bail!("the configured Vault token cannot be empty");
        }
        if namespace.as_deref().is_some_and(|value| {
            value.is_empty()
                || value.len() > 256
                || value.chars().any(|character| character.is_control())
        }) {
            anyhow::bail!("AGENT_RUNTIME_VAULT_NAMESPACE is invalid");
        }
        base_url.set_path("/");
        Ok(VaultSecretResolver {
            base_url,
            token,
            namespace,
            timeout_seconds: timeout_seconds.clamp(1, 30),
        })
    }

    fn resolve(&self, reference: &str) -> AppResult<String> {
        if reference.len() > 768 || reference.chars().any(char::is_whitespace) {
            return Err(AppError::Validation(
                "runtime provider secret reference is invalid".into(),
            ));
        }
        let (scheme, target) = reference.split_once(':').ok_or_else(|| {
            AppError::Validation("runtime provider secret reference is invalid".into())
        })?;
        match scheme {
            "env" => self.resolve_env(target),
            "secret" => self.resolve_configured_secret(target),
            "vault" => self.resolve_vault(target),
            _ => Err(AppError::Validation(
                "runtime provider secret reference scheme is unsupported".into(),
            )),
        }
    }

    fn resolve_env(&self, name: &str) -> AppResult<String> {
        Self::validate_env_name(name)?;
        std::env::var(name)
            .ok()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                AppError::Validation(format!(
                    "runtime provider secret environment variable is unavailable: {name}"
                ))
            })
    }

    fn resolve_configured_secret(&self, name: &str) -> AppResult<String> {
        Self::validate_secret_name(name)?;
        self.secrets.get(name).cloned().ok_or_else(|| {
            AppError::Validation(format!(
                "runtime provider configured secret is unavailable: {name}"
            ))
        })
    }

    fn resolve_vault(&self, target: &str) -> AppResult<String> {
        let (path, field) = target.split_once('#').ok_or_else(|| {
            AppError::Validation("runtime Vault reference must use vault:path#field".into())
        })?;
        Self::validate_vault_path(path)?;
        Self::validate_secret_name(field)?;
        let vault = self.vault.as_ref().ok_or_else(|| {
            AppError::Validation("runtime Vault resolver is not configured".into())
        })?;
        let mut endpoint = vault.base_url.clone();
        endpoint.set_path(&format!("/v1/{path}"));
        let client = Client::builder()
            .timeout(Duration::from_secs(vault.timeout_seconds))
            .redirect(Policy::none())
            .build()
            .map_err(|_| AppError::Validation("runtime Vault HTTP client failed".into()))?;
        let mut request = client.get(endpoint).header("X-Vault-Token", &vault.token);
        if let Some(namespace) = &vault.namespace {
            request = request.header("X-Vault-Namespace", namespace);
        }
        let response = request
            .send()
            .map_err(|_| AppError::Validation("runtime Vault request failed".into()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(AppError::Validation(format!(
                "runtime Vault returned HTTP {}",
                status.as_u16()
            )));
        }
        let body = Self::read_limited_response(
            response,
            MAX_VAULT_RESPONSE_BYTES,
            "runtime Vault response",
        )?;
        let response_json: Value = serde_json::from_slice(&body)
            .map_err(|_| AppError::Validation("runtime Vault response JSON is invalid".into()))?;
        response_json
            .pointer("/data/data")
            .or_else(|| response_json.pointer("/data"))
            .and_then(|data| data.get(field))
            .and_then(Value::as_str)
            .filter(|secret| !secret.is_empty())
            .map(str::to_string)
            .ok_or_else(|| {
                AppError::Validation(format!("runtime Vault field is unavailable: {field}"))
            })
    }

    fn read_limited_response(
        response: reqwest::blocking::Response,
        max_bytes: usize,
        label: &str,
    ) -> AppResult<Vec<u8>> {
        if response
            .content_length()
            .is_some_and(|length| length > max_bytes as u64)
        {
            return Err(AppError::Validation(format!(
                "{label} exceeds the configured size limit"
            )));
        }
        let mut body = Vec::new();
        response
            .take(max_bytes as u64 + 1)
            .read_to_end(&mut body)
            .map_err(|_| AppError::Validation(format!("{label} read failed")))?;
        if body.len() > max_bytes {
            return Err(AppError::Validation(format!(
                "{label} exceeds the configured size limit"
            )));
        }
        Ok(body)
    }

    fn validate_env_name(name: &str) -> AppResult<()> {
        let mut characters = name.chars();
        let valid = name.len() <= 128
            && characters
                .next()
                .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
            && characters.all(|character| character.is_ascii_alphanumeric() || character == '_');
        if valid {
            Ok(())
        } else {
            Err(AppError::Validation(
                "runtime environment secret name is invalid".into(),
            ))
        }
    }

    fn validate_secret_name(name: &str) -> AppResult<()> {
        let valid = !name.is_empty()
            && name.len() <= 128
            && name.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
            });
        if valid {
            Ok(())
        } else {
            Err(AppError::Validation(
                "runtime configured secret name is invalid".into(),
            ))
        }
    }

    fn validate_vault_path(path: &str) -> AppResult<()> {
        let valid = !path.is_empty()
            && path.len() <= 512
            && !path.starts_with('/')
            && !path.ends_with('/')
            && !path.contains("..")
            && path.split('/').all(|segment| {
                !segment.is_empty()
                    && segment.chars().all(|character| {
                        character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
                    })
            });
        if valid {
            Ok(())
        } else {
            Err(AppError::Validation(
                "runtime Vault secret path is invalid".into(),
            ))
        }
    }
}

#[derive(Debug, Clone)]
pub struct OpenAiResponsesRuntimeModelProvider {
    base_url: String,
    secret_resolver: RuntimeSecretResolver,
}

impl OpenAiResponsesRuntimeModelProvider {
    pub fn new(base_url: String, allow_insecure_http: bool) -> anyhow::Result<Self> {
        Self::with_secret_resolver_config(
            base_url,
            allow_insecure_http,
            RuntimeSecretResolverConfig::default(),
        )
    }

    pub fn with_secret_resolver_config(
        base_url: String,
        allow_insecure_http: bool,
        secret_config: RuntimeSecretResolverConfig,
    ) -> anyhow::Result<Self> {
        let base_url = base_url.trim().trim_end_matches('/').to_string();
        let parsed = Url::parse(&base_url)?;
        if parsed.scheme() != "https" && !(allow_insecure_http && parsed.scheme() == "http") {
            anyhow::bail!(
                "AGENT_RUNTIME_OPENAI_BASE_URL must use https unless insecure provider HTTP is explicitly enabled"
            );
        }
        if !parsed.username().is_empty() || parsed.password().is_some() {
            anyhow::bail!("AGENT_RUNTIME_OPENAI_BASE_URL cannot contain credentials");
        }
        Ok(Self {
            base_url,
            secret_resolver: RuntimeSecretResolver::from_config(secret_config)?,
        })
    }

    fn plan_schema(
        allowed_actions: &[String],
        approval_required_actions: &[String],
        max_actions: usize,
        max_approval_requests: usize,
    ) -> Value {
        let nullable_string = || json!({ "type": ["string", "null"] });
        let nullable_integer = || json!({ "type": ["integer", "null"] });
        let action_enum = if allowed_actions.is_empty() {
            vec!["__no_direct_action__".to_string()]
        } else {
            allowed_actions.to_vec()
        };
        let approval_enum = if approval_required_actions.is_empty() {
            vec!["__no_approval_action__".to_string()]
        } else {
            approval_required_actions.to_vec()
        };
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["summary", "actions", "approval_requests"],
            "properties": {
                "summary": { "type": "string", "maxLength": 2000 },
                "actions": {
                    "type": "array",
                    "maxItems": max_actions,
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "required": [
                            "tool", "conversation_id", "content", "project_id", "task_id",
                            "summary", "progress_percent", "blockers", "next_steps"
                        ],
                        "properties": {
                            "tool": { "type": "string", "enum": action_enum },
                            "conversation_id": nullable_string(),
                            "content": nullable_string(),
                            "project_id": nullable_string(),
                            "task_id": nullable_string(),
                            "summary": nullable_string(),
                            "progress_percent": nullable_integer(),
                            "blockers": {
                                "type": "array",
                                "maxItems": 20,
                                "items": { "type": "string", "maxLength": 500 }
                            },
                            "next_steps": {
                                "type": "array",
                                "maxItems": 20,
                                "items": { "type": "string", "maxLength": 500 }
                            }
                        }
                    }
                },
                "approval_requests": {
                    "type": "array",
                    "maxItems": max_approval_requests,
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "required": [
                            "tool", "reason", "target_agent_id", "project_id", "task_id",
                            "assignee_agent_id", "display_name", "handle", "persona",
                            "org_unit_id", "job_title", "reports_to_membership_id",
                            "handoff_plan", "handoff_agent_id"
                        ],
                        "properties": {
                            "tool": { "type": "string", "enum": approval_enum },
                            "reason": { "type": "string", "maxLength": 500 },
                            "target_agent_id": nullable_string(),
                            "project_id": nullable_string(),
                            "task_id": nullable_string(),
                            "assignee_agent_id": nullable_string(),
                            "display_name": nullable_string(),
                            "handle": nullable_string(),
                            "persona": nullable_string(),
                            "org_unit_id": nullable_string(),
                            "job_title": nullable_string(),
                            "reports_to_membership_id": nullable_string(),
                            "handoff_plan": nullable_string(),
                            "handoff_agent_id": nullable_string()
                        }
                    }
                }
            }
        })
    }

    fn extract_output_text(response: &Value) -> Option<String> {
        response
            .get("output_text")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                response
                    .get("output")?
                    .as_array()?
                    .iter()
                    .filter_map(|item| item.get("content").and_then(Value::as_array))
                    .flatten()
                    .find_map(|content| {
                        (content.get("type").and_then(Value::as_str) == Some("output_text"))
                            .then(|| content.get("text").and_then(Value::as_str))
                            .flatten()
                            .map(str::to_string)
                    })
            })
    }

    fn extract_refusal(response: &Value) -> Option<String> {
        response
            .get("output")?
            .as_array()?
            .iter()
            .filter_map(|item| item.get("content").and_then(Value::as_array))
            .flatten()
            .find_map(|content| {
                (content.get("type").and_then(Value::as_str) == Some("refusal"))
                    .then(|| content.get("refusal").and_then(Value::as_str))
                    .flatten()
                    .map(str::to_string)
            })
    }
}

impl AgentRuntimeModelProvider for OpenAiResponsesRuntimeModelProvider {
    fn check_secret_reference(&self, provider_secret_ref: &str) -> AppResult<()> {
        let secret = self.secret_resolver.resolve(provider_secret_ref)?;
        if secret.is_empty() {
            return Err(AppError::Validation(
                "runtime provider secret resolved to an empty value".into(),
            ));
        }
        drop(secret);
        Ok(())
    }

    fn generate_plan(
        &self,
        request: AgentRuntimeModelRequest,
    ) -> AppResult<AgentRuntimeModelResponse> {
        if request.model_provider != AGENT_RUNTIME_MODEL_PROVIDER_OPENAI_RESPONSES {
            return Err(AppError::Validation(
                "runtime model provider is unsupported".into(),
            ));
        }
        let secret = self.secret_resolver.resolve(&request.provider_secret_ref)?;
        let client = Client::builder()
            .timeout(Duration::from_secs(request.timeout_seconds as u64))
            .redirect(Policy::none())
            .build()
            .map_err(|error| {
                AppError::Validation(format!("runtime model HTTP client failed: {error}"))
            })?;
        let context_json = serde_json::to_string(&request.context_snapshot).map_err(|error| {
            AppError::Validation(format!(
                "runtime model context serialization failed: {error}"
            ))
        })?;
        let payload = json!({
            "model": request.model_name,
            "store": false,
            "instructions": request.system_prompt,
            "input": format!(
                "请根据以下 context_snapshot 生成本轮 JSON 行动计划。只能使用 schema 中允许的工具和上下文内已有 ID。\n{context_json}"
            ),
            "max_output_tokens": request.max_output_tokens,
            "text": {
                "format": {
                    "type": "json_schema",
                    "name": "agent_runtime_plan",
                    "strict": true,
                    "schema": Self::plan_schema(
                        &request.allowed_actions,
                        &request.approval_required_actions,
                        request.max_actions,
                        request.max_approval_requests
                    )
                }
            }
        });
        let response = client
            .post(format!("{}/responses", self.base_url))
            .bearer_auth(secret)
            .json(&payload)
            .send()
            .map_err(|_| AppError::Validation("runtime model request failed".into()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(AppError::Validation(format!(
                "runtime model provider returned HTTP {}",
                status.as_u16()
            )));
        }
        let body = RuntimeSecretResolver::read_limited_response(
            response,
            MAX_PROVIDER_RESPONSE_BYTES,
            "runtime model response",
        )?;
        let response_json: Value = serde_json::from_slice(&body).map_err(|error| {
            AppError::Validation(format!("runtime model response JSON is invalid: {error}"))
        })?;
        if Self::extract_refusal(&response_json).is_some() {
            return Err(AppError::Validation(
                "runtime model refused to create a plan".into(),
            ));
        }
        let output_text = Self::extract_output_text(&response_json).ok_or_else(|| {
            AppError::Validation("runtime model response did not contain output_text".into())
        })?;
        let plan: AgentRuntimeModelPlan = serde_json::from_str(&output_text).map_err(|error| {
            AppError::Validation(format!("runtime model plan JSON is invalid: {error}"))
        })?;
        let usage = response_json.get("usage").cloned().unwrap_or_default();
        Ok(AgentRuntimeModelResponse {
            plan,
            provider_request_id: response_json
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string),
            resolved_model_name: response_json
                .get("model")
                .and_then(Value::as_str)
                .map(str::to_string),
            input_tokens: usage
                .get("input_tokens")
                .and_then(Value::as_i64)
                .unwrap_or(0),
            output_tokens: usage
                .get("output_tokens")
                .and_then(Value::as_i64)
                .unwrap_or(0),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread::{self, JoinHandle};

    use super::*;

    fn spawn_http_response(
        status: u16,
        reason: &'static str,
        body: &'static str,
    ) -> (String, JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake Vault");
        let address = listener.local_addr().expect("fake Vault address");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept fake Vault request");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("set read timeout");
            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];
            loop {
                let read = stream.read(&mut buffer).expect("read fake Vault request");
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .expect("write fake Vault response");
            String::from_utf8(request).expect("HTTP request is UTF-8")
        });
        (format!("http://{address}"), handle)
    }

    fn vault_resolver(
        address: String,
        token: &str,
        namespace: Option<&str>,
    ) -> RuntimeSecretResolver {
        RuntimeSecretResolver::from_config(RuntimeSecretResolverConfig {
            vault_addr: Some(address),
            vault_token: Some(token.to_string()),
            vault_namespace: namespace.map(str::to_string),
            allow_insecure_vault_http: true,
            vault_timeout_seconds: 2,
            ..RuntimeSecretResolverConfig::default()
        })
        .expect("build Vault resolver")
    }

    #[test]
    fn resolves_configured_secret_without_exposing_it_in_debug_output() {
        let config = RuntimeSecretResolverConfig {
            secrets_json: Some(r#"{"MODEL_KEY":"configured-super-secret"}"#.to_string()),
            ..RuntimeSecretResolverConfig::default()
        };
        assert!(!format!("{config:?}").contains("configured-super-secret"));
        let resolver =
            RuntimeSecretResolver::from_config(config).expect("build configured secret resolver");

        assert_eq!(
            resolver
                .resolve("secret:MODEL_KEY")
                .expect("resolve secret"),
            "configured-super-secret"
        );
        assert!(!format!("{resolver:?}").contains("configured-super-secret"));
    }

    #[test]
    fn resolves_existing_environment_secret() {
        let resolver = RuntimeSecretResolver::from_config(RuntimeSecretResolverConfig::default())
            .expect("build environment resolver");

        assert_eq!(
            resolver.resolve("env:PATH").expect("resolve PATH"),
            std::env::var("PATH").expect("PATH is configured")
        );
    }

    #[test]
    fn rejects_invalid_config_without_echoing_secret_values() {
        let error = RuntimeSecretResolver::from_config(RuntimeSecretResolverConfig {
            secrets_json: Some(r#"{"MODEL_KEY":{"value":"configured-super-secret"}}"#.to_string()),
            ..RuntimeSecretResolverConfig::default()
        })
        .expect_err("non-string secret must fail");

        assert!(!error.to_string().contains("configured-super-secret"));
    }

    #[test]
    fn resolves_vault_kv_v1_and_sends_auth_headers() {
        let (address, server) =
            spawn_http_response(200, "OK", r#"{"data":{"model_key":"vault-v1-secret"}}"#);
        let resolver = vault_resolver(address, "vault-test-token", Some("company-a"));

        assert_eq!(
            resolver
                .resolve("vault:secret/ai-chat#model_key")
                .expect("resolve Vault KV v1 secret"),
            "vault-v1-secret"
        );
        let request = server.join().expect("fake Vault server completed");
        assert!(request.starts_with("GET /v1/secret/ai-chat HTTP/1.1"));
        assert!(request
            .to_ascii_lowercase()
            .contains("x-vault-token: vault-test-token"));
        assert!(request
            .to_ascii_lowercase()
            .contains("x-vault-namespace: company-a"));
    }

    #[test]
    fn resolves_vault_kv_v2() {
        let (address, server) = spawn_http_response(
            200,
            "OK",
            r#"{"data":{"data":{"model_key":"vault-v2-secret"},"metadata":{"version":1}}}"#,
        );
        let resolver = vault_resolver(address, "vault-test-token", None);

        assert_eq!(
            resolver
                .resolve("vault:secret/data/ai-chat#model_key")
                .expect("resolve Vault KV v2 secret"),
            "vault-v2-secret"
        );
        server.join().expect("fake Vault server completed");
    }

    #[test]
    fn vault_errors_do_not_echo_response_or_token() {
        let (address, server) = spawn_http_response(
            500,
            "Internal Server Error",
            r#"{"error":"vault-test-token and leaked-provider-secret"}"#,
        );
        let resolver = vault_resolver(address, "vault-test-token", None);

        let error = resolver
            .resolve("vault:secret/data/ai-chat#model_key")
            .expect_err("Vault error must fail");
        let message = error.to_string();
        assert!(message.contains("HTTP 500"));
        assert!(!message.contains("vault-test-token"));
        assert!(!message.contains("leaked-provider-secret"));
        server.join().expect("fake Vault server completed");
    }

    #[test]
    fn rejects_unsafe_secret_references_and_insecure_vault_by_default() {
        let resolver = RuntimeSecretResolver::from_config(RuntimeSecretResolverConfig::default())
            .expect("build empty resolver");
        for reference in [
            "secret:",
            "env:MODEL KEY",
            "vault:secret/../admin#key",
            "vault:secret/data/ai-chat?version=1#key",
            "vault:/secret/data/ai-chat#key",
        ] {
            assert!(
                resolver.resolve(reference).is_err(),
                "{reference} must fail"
            );
        }

        let error = RuntimeSecretResolver::from_config(RuntimeSecretResolverConfig {
            vault_addr: Some("http://127.0.0.1:8200".to_string()),
            vault_token: Some("vault-test-token".to_string()),
            ..RuntimeSecretResolverConfig::default()
        })
        .expect_err("insecure Vault HTTP must be opt-in");
        assert!(error.to_string().contains("must use https"));
        assert!(!error.to_string().contains("vault-test-token"));
    }
}

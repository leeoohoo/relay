import { FormEvent, useEffect, useState } from "react";
import { api } from "../../api/client";
import { Field } from "../../components/ui";
import type { CompanyProject, ProjectGitAdminView } from "../../types/platform";

export function ProjectGitCard(props: {
  companyId: string;
  project: CompanyProject;
  token: string;
  canManage: boolean;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const [remoteUrl, setRemoteUrl] = useState(props.project.git?.remote_url ?? "");
  const [hostLocalPath, setHostLocalPath] = useState("");
  const [defaultBranch, setDefaultBranch] = useState(props.project.git?.default_branch ?? "main");
  const [githubToken, setGithubToken] = useState("");
  const [githubTokenConfigured, setGithubTokenConfigured] = useState(false);
  const [managedTokenConfigured, setManagedTokenConfigured] = useState(false);
  const [branchPrefix, setBranchPrefix] = useState(props.project.git?.branch_prefix ?? "relay/");
  const [allowAgentPush, setAllowAgentPush] = useState(props.project.git?.push_enabled ?? false);
  const [configured, setConfigured] = useState(Boolean(props.project.git));
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!props.canManage) return;
    let active = true;
    api<{ git: ProjectGitAdminView | null; github_token_configured: boolean; managed_token_configured: boolean }>(
      `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/git`,
      {},
      props.token,
    )
      .then(({ git, github_token_configured, managed_token_configured }) => {
        if (!active) return;
        setRemoteUrl(git?.remote_url ?? "");
        setHostLocalPath(git?.host_local_path ?? "");
        setDefaultBranch(git?.default_branch ?? "main");
        setGithubToken("");
        setGithubTokenConfigured(github_token_configured);
        setManagedTokenConfigured(managed_token_configured);
        setBranchPrefix(git?.branch_prefix ?? "relay/");
        setAllowAgentPush(git?.allow_agent_push ?? false);
        setConfigured(Boolean(git));
      })
      .catch(props.onError);
    return () => { active = false; };
  }, [props.canManage, props.companyId, props.project.project.id, props.token]);

  async function saveGit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    try {
      const response = await api<{ git: ProjectGitAdminView; github_token_configured: boolean; managed_token_configured: boolean }>(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/git`,
        {
          method: "PUT",
          body: JSON.stringify({
            remote_url: remoteUrl,
            host_local_path: hostLocalPath || null,
            default_branch: defaultBranch || null,
            github_token: githubToken || null,
            clear_github_token: false,
            allow_agent_push: allowAgentPush,
            branch_prefix: branchPrefix || null,
          }),
        },
        props.token,
      );
      setRemoteUrl(response.git.remote_url);
      setHostLocalPath(response.git.host_local_path);
      setDefaultBranch(response.git.default_branch);
      setGithubToken("");
      setGithubTokenConfigured(response.github_token_configured);
      setManagedTokenConfigured(response.managed_token_configured);
      setBranchPrefix(response.git.branch_prefix);
      setAllowAgentPush(response.git.allow_agent_push);
      setConfigured(true);
      props.onNotice(`${props.project.project.name} 的 Git 配置已保存`);
      await props.onChanged();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function clearGithubToken() {
    if (!window.confirm("清除这个项目已保存的 GitHub Token？公开仓库仍可拉取，但私有仓库和 Push 将不可用。")) return;
    setBusy(true);
    try {
      const response = await api<{ git: ProjectGitAdminView; github_token_configured: boolean; managed_token_configured: boolean }>(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/git`,
        {
          method: "PUT",
          body: JSON.stringify({
            remote_url: remoteUrl,
            host_local_path: hostLocalPath || null,
            default_branch: defaultBranch || null,
            github_token: null,
            clear_github_token: true,
            allow_agent_push: allowAgentPush,
            branch_prefix: branchPrefix || null,
          }),
        },
        props.token,
      );
      setGithubToken("");
      setGithubTokenConfigured(response.github_token_configured);
      setManagedTokenConfigured(response.managed_token_configured);
      props.onNotice(`${props.project.project.name} 的 GitHub Token 已清除`);
      await props.onChanged();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function clearGit() {
    if (!window.confirm(`清除 ${props.project.project.name} 的 Git 配置？Agent 将无法再从 Relay 获取仓库地址。`)) return;
    setBusy(true);
    try {
      await api(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/git`,
        { method: "DELETE" },
        props.token,
      );
      setRemoteUrl("");
      setHostLocalPath("");
      setDefaultBranch("main");
      setGithubToken("");
      setGithubTokenConfigured(false);
      setManagedTokenConfigured(false);
      setBranchPrefix("relay/");
      setAllowAgentPush(false);
      setConfigured(false);
      props.onNotice(`${props.project.project.name} 的 Git 配置已清除`);
      await props.onChanged();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  return (
    <article className="project-git-editor">
      {props.canManage ? (
        <form className="project-git-form" onSubmit={saveGit}>
          <Field label="Git Remote URL">
            <input
              value={remoteUrl}
              onChange={(event) => setRemoteUrl(event.target.value)}
              placeholder="https://github.com/owner/repository.git"
              required
            />
          </Field>
          <div className="managed-workspace-note">
            <div>
              <strong>Agent 工作区由 Relay 自动管理</strong>
              <span>默认会按项目生成独立目录，你只需要填写 Git 地址。</span>
            </div>
            {configured && hostLocalPath ? <code>{hostLocalPath}</code> : null}
          </div>
          <details className="project-git-advanced">
            <summary>高级设置：自定义宿主机目录</summary>
            <Field label="宿主机项目根目录">
              <input
                value={hostLocalPath}
                onChange={(event) => setHostLocalPath(event.target.value)}
                placeholder="留空时由 Relay 自动生成"
              />
            </Field>
            <small>仅当 Codex 必须使用指定的现有目录时才需要设置。修改已有目录不会自动搬迁旧工作区。</small>
          </details>
          <div className="form-grid">
            <Field label="默认分支">
              <input value={defaultBranch} onChange={(event) => setDefaultBranch(event.target.value)} placeholder="main" />
            </Field>
            <Field label="Agent 分支前缀">
              <input value={branchPrefix} onChange={(event) => setBranchPrefix(event.target.value)} placeholder="relay/" />
            </Field>
          </div>
          <Field label={managedTokenConfigured ? "Git Token（Relay 已自动配置）" : "GitHub Token（私有仓库或需要 Push 时填写）"}>
            <input
              type="password"
              autoComplete="new-password"
              value={githubToken}
              onChange={(event) => setGithubToken(event.target.value)}
              placeholder={managedTokenConfigured ? "托管凭证已就绪；填写新 Token 可手动覆盖" : githubTokenConfigured ? "Token 已保存，留空表示不修改" : "github_pat_..."}
            />
          </Field>
          {managedTokenConfigured ? (
            <div className="project-git-actions">
              <span className="git-security-note">项目专用 Token 已由 Relay 自动创建并保存在宿主机</span>
            </div>
          ) : githubTokenConfigured ? (
            <div className="project-git-actions">
              <span className="git-security-note">Token 已安全保存在宿主机</span>
              <button className="button small" type="button" onClick={() => void clearGithubToken()} disabled={busy}>清除 Token</button>
            </div>
          ) : null}
          <label className="check-row">
            <input type="checkbox" checked={allowAgentPush} onChange={(event) => setAllowAgentPush(event.target.checked)} />
            允许 Codex 在完成验证后 push 自己的工作分支
          </label>
          <div className="git-security-note">
            Git 地址请使用 HTTPS。Token 只保存在宿主机本地凭证目录，不写入项目数据库，也不会通过 MCP 返回；配置托管代码平台后，Agent 可以自行创建仓库与项目专用 Token。
          </div>
          <div className="project-git-actions">
            {configured ? <button className="button small" type="button" onClick={() => void clearGit()} disabled={busy}>清除配置</button> : null}
            <button className="button primary small" disabled={busy}>{busy ? "正在保存…" : "保存 Git 配置"}</button>
          </div>
        </form>
      ) : (
        <div className="project-git-readonly">
          {props.project.git ? (
            <>
              <code>{props.project.git.remote_url}</code>
              <span>默认分支 {props.project.git.default_branch}</span>
            </>
          ) : <span>请联系公司 Owner/Admin 配置仓库。</span>}
        </div>
      )}
    </article>
  );
}

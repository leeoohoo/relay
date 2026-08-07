import { Icon } from "../../components/ui";
import type { CompanyProject } from "../../types/platform";

export function ProjectGitCard(props: {
  companyId: string;
  project: CompanyProject;
  token: string;
  onError: (error: unknown) => void;
}) {
  const git = props.project.git;
  if (!git) {
    return (
      <div className="project-git-readonly project-git-missing">
        <Icon name="alert" />
        <span>Harness 仓库尚未初始化完成</span>
      </div>
    );
  }

  return (
    <article className="project-git-managed-card">
      <div className="project-git-managed-url">
        <span className="project-repository-icon"><Icon name="git" /></span>
        <div><small>HARNESS REPOSITORY</small><code>{git.remote_url}</code></div>
        <span className="git-config-state configured">系统托管 · 只读</span>
      </div>
      <div className="project-git-managed-grid">
        <div><small>默认分支</small><strong>{git.default_branch}</strong></div>
        <div><small>Agent 分支</small><strong>{git.branch_prefix}*</strong></div>
        <div><small>推送权限</small><strong>{git.push_enabled ? "已启用" : "已关闭"}</strong></div>
        <div><small>项目凭证</small><strong>{git.auth_configured ? "已就绪" : "待恢复"}</strong></div>
      </div>
    </article>
  );
}

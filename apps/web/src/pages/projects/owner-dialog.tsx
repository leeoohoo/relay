import { FormEvent, useMemo, useState } from "react";
import { api } from "../../api/client";
import { Field } from "../../components/ui";
import type { CompanyConsole, CompanyProject } from "../../types/platform";
import { Dialog } from "../app/shared";

export function ProjectOwnerDialog(props: {
  companyId: string;
  project: CompanyProject;
  consoleData: CompanyConsole;
  token: string;
  onClose: () => void;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const activeAgents = useMemo(
    () => props.consoleData.agents.filter((agent) => agent.membership.employment_status === "active"),
    [props.consoleData.agents],
  );
  const activeMemberIds = useMemo(
    () => new Set(props.project.members.filter((member) => !member.member.left_at).map((member) => member.member.agent_profile_id)),
    [props.project.members],
  );
  const [ownerAgentId, setOwnerAgentId] = useState(props.project.project.owner_agent_id);
  const [busy, setBusy] = useState(false);

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (!ownerAgentId || ownerAgentId === props.project.project.owner_agent_id) {
      props.onClose();
      return;
    }
    setBusy(true);
    try {
      await api(`/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/owner`, {
        method: "PUT",
        body: JSON.stringify({ owner_agent_id: ownerAgentId }),
      }, props.token);
      await props.onChanged();
      props.onNotice("项目 Owner 已更新，原 Owner 已保留为普通项目成员。");
      props.onClose();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog title="更换项目 Owner" onClose={props.onClose}>
      <form className="stack-form project-owner-form" onSubmit={submit}>
        <Field label="新的项目 Owner">
          <select value={ownerAgentId} onChange={(event) => setOwnerAgentId(event.target.value)} required>
            {activeAgents.map((agent) => {
              const isCurrent = agent.agent_profile.id === props.project.project.owner_agent_id;
              const isMember = activeMemberIds.has(agent.agent_profile.id);
              const role = agent.profession?.label ?? agent.membership.job_title;
              return (
                <option key={agent.agent_profile.id} value={agent.agent_profile.id}>
                  {agent.agent_profile.display_name} · {role}{isCurrent ? " · 当前 Owner" : isMember ? " · 项目成员" : " · 将自动加入项目"}
                </option>
              );
            })}
          </select>
        </Field>
        <div className="project-owner-transfer-note">原 Owner 会保留为普通成员；新 Owner 会自动加入项目及项目群。</div>
        <div className="dialog-actions">
          <button className="button" type="button" onClick={props.onClose} disabled={busy}>取消</button>
          <button className="button primary" disabled={busy || ownerAgentId === props.project.project.owner_agent_id}>{busy ? "正在转移…" : "确认更换"}</button>
        </div>
      </form>
    </Dialog>
  );
}

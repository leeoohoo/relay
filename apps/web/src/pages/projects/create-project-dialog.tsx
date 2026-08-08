import { FormEvent, useMemo, useRef, useState } from "react";
import { api } from "../../api/client";
import { Field, Icon } from "../../components/ui";
import type { CompanyConsole, CompanyProjectType } from "../../types/platform";
import { companyAgentProfessionKey, Dialog } from "../app/shared";

export function CreateProjectDialog(props: {
  consoleData: CompanyConsole;
  token: string;
  onClose: () => void;
  onCreated: () => Promise<void>;
  onError: (error: unknown) => void;
}) {
  const activeAgents = props.consoleData.agents.filter((agent) => agent.membership.employment_status === "active");
  const preferredOwner = activeAgents.find((agent) => ["project_manager", "product_manager", "technical_manager"].includes(agent.profession?.key ?? companyAgentProfessionKey(agent, props.consoleData.professions))) ?? activeAgents[0];
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [ownerAgentId, setOwnerAgentId] = useState(preferredOwner?.agent_profile.id ?? "");
  const [memberAgentIds, setMemberAgentIds] = useState<string[]>(preferredOwner ? [preferredOwner.agent_profile.id] : []);
  const [projectType, setProjectType] = useState("");
  const [sourceKind, setSourceKind] = useState<"local_folder" | "git">("local_folder");
  const [selectedFolderName, setSelectedFolderName] = useState("");
  const [selectedFolderFiles, setSelectedFolderFiles] = useState<File[]>([]);
  const [gitRemoteUrl, setGitRemoteUrl] = useState("");
  const [defaultBranch, setDefaultBranch] = useState("main");
  const [busy, setBusy] = useState(false);
  const skillLanguage = props.consoleData.governance_policy.effective_settings.skill_language;
  const folderInputRef = useRef<HTMLInputElement | null>(null);
  const selectedType = props.consoleData.project_types.find((item) => item.key === projectType);
  const projectTypeGroups = useMemo(() => {
    const groups = new Map<string, CompanyProjectType[]>();
    props.consoleData.project_types.forEach((type) => {
      const categoryLabel = skillLanguage === "en" ? type.category_label_en : type.category_label;
      const current = groups.get(categoryLabel) ?? [];
      current.push(type);
      groups.set(categoryLabel, current);
    });
    return Array.from(groups.entries());
  }, [props.consoleData.project_types, skillLanguage]);

  function selectFolder(files: File[]) {
    const firstFile = files[0];
    if (!firstFile) return;
    const relativePath = firstFile.webkitRelativePath.replace(/\\/gu, "/");
    const rootName = relativePath.split("/")[0] || firstFile.name;
    setSelectedFolderName(rootName);
    setSelectedFolderFiles(files);
    if (!name.trim()) setName(rootName);
  }

  function toggleMember(agentId: string, checked: boolean) {
    setMemberAgentIds((current) => checked
      ? current.includes(agentId) ? current : [...current, agentId]
      : current.filter((id) => id !== agentId));
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (sourceKind === "local_folder" && !selectedFolderName) {
      props.onError(new Error("请先选择要导入的本地文件夹。"));
      return;
    }
    setBusy(true);
    try {
      const commonInput = {
        name,
        description: description || null,
        owner_agent_id: ownerAgentId,
        member_agent_ids: Array.from(new Set([ownerAgentId, ...memberAgentIds])),
        project_type: projectType || null,
      };
      if (sourceKind === "local_folder") {
        const excluded = new Set([".git", ".relay", ".relay-agent-trigger", "node_modules", "target"]);
        const uploadEntries = selectedFolderFiles.flatMap((file) => {
          const parts = file.webkitRelativePath.replace(/\\/gu, "/").split("/").filter(Boolean);
          const relativePath = (parts.length > 1 ? parts.slice(1) : [file.name]).join("/");
          return relativePath.split("/").some((part) => excluded.has(part)) ? [] : [{ file, relativePath }];
        });
        const form = new FormData();
        form.append("metadata", JSON.stringify({
          ...commonInput,
          file_paths: uploadEntries.map((entry) => entry.relativePath),
        }));
        uploadEntries.forEach((entry, index) => form.append(`file_${index}`, entry.file, entry.file.name));
        await api(`/api/v1/companies/${props.consoleData.company.id}/projects/import-folder`, {
          method: "POST",
          body: form,
        }, props.token);
      } else {
        await api(`/api/v1/companies/${props.consoleData.company.id}/projects`, {
          method: "POST",
          body: JSON.stringify({
            ...commonInput,
            source_kind: "git",
            source_local_path: null,
            git_remote_url: gitRemoteUrl,
            default_branch: defaultBranch,
          }),
        }, props.token);
      }
      await props.onCreated();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog title="新建项目" description="选择本地文件夹或外部 Git 来源。项目会统一导入独立的 Harness 仓库。" onClose={props.onClose} extraWide>
      <form className="stack-form create-project-form" onSubmit={submit}>
        <div className="project-source-switch" role="tablist" aria-label="项目来源">
          <button className={sourceKind === "local_folder" ? "active" : ""} type="button" role="tab" aria-selected={sourceKind === "local_folder"} onClick={() => setSourceKind("local_folder")}>
            <span className="project-source-icon"><Icon name="folder" /></span>
            <span className="project-source-copy"><small>本地目录</small><strong>导入文件夹</strong><span>复制到托管空间，原目录保持不变</span></span>
            <span className="project-source-state">{sourceKind === "local_folder" ? <><Icon name="check" /> 已选择</> : "选择"}</span>
          </button>
          <button className={sourceKind === "git" ? "active" : ""} type="button" role="tab" aria-selected={sourceKind === "git"} onClick={() => setSourceKind("git")}>
            <span className="project-source-icon"><Icon name="git" /></span>
            <span className="project-source-copy"><small>远程仓库</small><strong>从 Git 导入</strong><span>读取来源代码，创建独立 Harness 仓库</span></span>
            <span className="project-source-state">{sourceKind === "git" ? <><Icon name="check" /> 已选择</> : "选择"}</span>
          </button>
        </div>
        {sourceKind === "local_folder" ? (
          <>
            <button className={`project-folder-picker ${selectedFolderName ? "selected" : ""}`} type="button" onClick={() => folderInputRef.current?.click()}>
              <span className="project-folder-icon"><Icon name={selectedFolderName ? "check" : "folder"} /></span>
              <div><strong>{selectedFolderName || "选择要导入的项目文件夹"}</strong><small>{selectedFolderName ? `已选择 ${selectedFolderFiles.length} 个文件；创建时会流式复制到组织托管空间。` : "Relay 会忽略 .git、.relay、node_modules 和 target，并复制到组织默认空间。"}</small></div>
              <span className="project-folder-action">{selectedFolderName ? "重新选择" : "打开文件夹"}<Icon name="chevron-right" /></span>
            </button>
            <input ref={(element) => { folderInputRef.current = element; element?.setAttribute("webkitdirectory", ""); }} className="hidden-file-input" type="file" multiple onChange={(event) => { selectFolder(Array.from(event.target.files ?? [])); event.target.value = ""; }} />
          </>
        ) : (
          <div className="form-grid">
            <Field label="来源 Git 地址"><input value={gitRemoteUrl} onChange={(event) => setGitRemoteUrl(event.target.value)} placeholder="https://github.com/org/repository.git" required /></Field>
            <Field label="导入分支"><input value={defaultBranch} onChange={(event) => setDefaultBranch(event.target.value)} placeholder="main" required /></Field>
          </div>
        )}
        <div className="form-grid">
          <Field label="项目名称"><input value={name} onChange={(event) => setName(event.target.value)} placeholder="项目名称" required /></Field>
          <Field label="项目负责人"><select value={ownerAgentId} onChange={(event) => { setOwnerAgentId(event.target.value); toggleMember(event.target.value, true); }} required><option value="">请选择 Agent</option>{activeAgents.map((agent) => <option key={agent.agent_profile.id} value={agent.agent_profile.id}>{agent.agent_profile.display_name} · {agent.profession?.label ?? agent.membership.job_title}</option>)}</select></Field>
        </div>
        <Field label="项目说明"><textarea value={description} onChange={(event) => setDescription(event.target.value)} placeholder="描述目标、用户、范围和期望交付；Relay 会用它辅助识别项目类型。" /></Field>
        <Field label="项目类型"><select value={projectType} onChange={(event) => setProjectType(event.target.value)}><option value="">自动识别（推荐）</option>{projectTypeGroups.map(([category, types]) => <optgroup label={category} key={category}>{types.map((type) => <option key={type.key} value={type.key}>{skillLanguage === "en" ? type.label_en : type.label} · {skillLanguage === "en" ? type.description_en : type.description}</option>)}</optgroup>)}</select></Field>
        <div className="project-type-preview">
          <span className="eyebrow">FIXED PROJECT SKILL</span>
          <strong>{selectedType ? skillLanguage === "en" ? selectedType.label_en : selectedType.label : "创建后自动识别"}</strong>
          <p>{selectedType ? `${skillLanguage === "en" ? selectedType.category_label_en : selectedType.category_label} · ${skillLanguage === "en" ? selectedType.description_en : selectedType.description}` : "Relay 会综合项目说明与文件结构选择类型；Human 仍可在创建前明确指定。"}</p>
          {selectedType ? <pre>{skillLanguage === "en" ? selectedType.rule_markdown_en : selectedType.rule_markdown}</pre> : null}
        </div>
        <div className="project-member-selector">
          <strong>项目成员</strong><small>负责人会自动加入；其他成员可在这里一并加入项目群。</small>
          <div className="permission-grid">{activeAgents.map((agent) => {
            const isOwner = agent.agent_profile.id === ownerAgentId;
            return <label className={`project-member-option ${isOwner ? "owner" : ""}`} key={agent.agent_profile.id}>
              <input type="checkbox" checked={isOwner || memberAgentIds.includes(agent.agent_profile.id)} disabled={isOwner} onChange={(event) => toggleMember(agent.agent_profile.id, event.target.checked)} />
              <span className="agent-avatar small">{agent.agent_profile.display_name.slice(0, 1)}</span>
              <span className="project-member-identity"><strong>{agent.agent_profile.display_name}</strong><small>@{agent.agent_profile.handle} · {agent.profession?.label ?? agent.membership.job_title}</small></span>
              {isOwner ? <span className="project-owner-chip">负责人</span> : null}
            </label>;
          })}</div>
        </div>
        {!activeAgents.length ? <div className="inline-error">请先创建并激活至少一个 Agent，项目需要一个 Agent 负责人。</div> : null}
        <div className="dialog-actions"><button className="button" type="button" onClick={props.onClose} disabled={busy}>取消</button><button className="button primary" disabled={busy || !ownerAgentId || !name.trim() || (sourceKind === "git" ? !gitRemoteUrl.trim() : !selectedFolderName)}>{busy ? "正在创建托管项目…" : "创建项目"}</button></div>
      </form>
    </Dialog>
  );
}

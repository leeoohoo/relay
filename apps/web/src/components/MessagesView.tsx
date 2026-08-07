import { ClipboardEvent, FormEvent, Fragment, ReactNode, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { api } from "../api/client";
import type { CompanyRealtimeEvent } from "../api/types";
import { MessageAttachments } from "./MessageAttachments";
import { MessageHistoryControl } from "./MessageHistoryControl";
import { GroupMembersDrawer } from "./GroupMembersDrawer";
import { Pagination, usePagination } from "./Pagination";
import { Field, Icon } from "./ui";
import { useConversationMessages } from "../hooks/useConversationMessages";
import type { HumanUser } from "../types/appShell";
import type { Conversation, Message, PendingMessageFile } from "../types/chat";
import type { CompanyAgent, CompanyConsole } from "../types/platform";

export function MessagesView(props: {
  consoleData: CompanyConsole;
  humanUser: HumanUser;
  token: string;
  realtimeEvent: CompanyRealtimeEvent | null;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (message: string) => void;
}) {
  const [selectedId, setSelectedId] = useState(props.consoleData.conversations[0]?.preview.id ?? "");
  const [conversationQuery, setConversationQuery] = useState("");
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [showNewDirect, setShowNewDirect] = useState(false);
  const [showMemberDetails, setShowMemberDetails] = useState(false);
  const [targetAgentId, setTargetAgentId] = useState("");
  const [mentionedAgentIds, setMentionedAgentIds] = useState<string[]>([]);
  const [mentionAll, setMentionAll] = useState(false);
  const [mentionQuery, setMentionQuery] = useState<string | null>(null);
  const [pendingFiles, setPendingFiles] = useState<PendingMessageFile[]>([]);
  const messageStreamRef = useRef<HTMLDivElement | null>(null);
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const shouldScrollToLatestRef = useRef(true);
  const isNearLatestRef = useRef(true);
  const historyScrollAnchorRef = useRef<{ height: number; top: number } | null>(null);
  const selected = props.consoleData.conversations.find((item) => item.preview.id === selectedId);
  const selectedProject = selected?.context.project_id
    ? props.consoleData.projects.find((project) => project.project.id === selected.context.project_id)
    : null;
  const selectedProjectPaused = selectedProject?.project.status === "paused";
  const canSend = props.consoleData.human_membership.status === "active"
    && ["owner", "admin"].includes(props.consoleData.human_membership.role);
  const activeAgents = props.consoleData.agents.filter((agent) => agent.membership.employment_status === "active");
  const agentNames = useMemo(() => new Map(props.consoleData.agents.map((agent) => [agent.agent_profile.id, agent.agent_profile.display_name])), [props.consoleData.agents]);
  const agentDirectory = useMemo(() => new Map(props.consoleData.agents.map((agent) => [agent.agent_profile.id, agent])), [props.consoleData.agents]);
  const visibleConversations = useMemo(() => {
    const query = conversationQuery.trim().toLocaleLowerCase();
    if (!query) return props.consoleData.conversations;
    return props.consoleData.conversations.filter((conversation) => {
      const title = conversationDisplayTitle(conversation, agentNames);
      return title.toLocaleLowerCase().includes(query)
        || (conversation.preview.last_message_preview ?? "").toLocaleLowerCase().includes(query);
    });
  }, [agentNames, conversationQuery, props.consoleData.conversations]);
  const conversationPagination = usePagination(visibleConversations, 10, conversationQuery);
  const selectedConversationAgents = (selected?.member_agent_ids ?? []).flatMap((agentId) => {
    const agent = agentDirectory.get(agentId);
    return agent ? [agent] : [];
  });
  const selectedIsGroup = selected?.preview.conversation_type === "group";
  const selectedIsDirect = selected?.preview.conversation_type === "direct";
  const mentionCandidates = selectedIsGroup && mentionQuery !== null
    ? selectedConversationAgents.filter((agent) => {
      const query = mentionQuery.trim().toLocaleLowerCase();
      return !query
        || agent.agent_profile.display_name.toLocaleLowerCase().includes(query)
        || agent.agent_profile.handle.replace(/^@/, "").toLocaleLowerCase().includes(query);
    })
    : [];
  const showMentionAllCandidate = mentionQuery !== null
    && (!mentionQuery.trim() || "所有人".includes(mentionQuery.trim()));

  const {
    messages,
    hasMore: hasOlderMessages,
    loading: loadingMessages,
    loadingOlder,
    loadOlder,
    refreshLatest,
  } = useConversationMessages<Message>({
    conversationId: selectedId,
    token: props.token,
    realtimeEvent: props.realtimeEvent,
    onError: props.onError,
  });

  useEffect(() => {
    setSelectedId((current) => current && props.consoleData.conversations.some((item) => item.preview.id === current)
      ? current
      : props.consoleData.conversations[0]?.preview.id ?? "");
  }, [props.consoleData.conversations]);

  useEffect(() => {
    setShowMemberDetails(false);
    setMentionedAgentIds([]);
    setMentionAll(false);
    setMentionQuery(null);
    setPendingFiles([]);
    shouldScrollToLatestRef.current = true;
    isNearLatestRef.current = true;
  }, [selectedId, props.token]);

  const latestMessageId = messages[messages.length - 1]?.id ?? "";
  useLayoutEffect(() => {
    const stream = messageStreamRef.current;
    const historyAnchor = historyScrollAnchorRef.current;
    if (stream && historyAnchor) {
      stream.scrollTop = historyAnchor.top + stream.scrollHeight - historyAnchor.height;
      historyScrollAnchorRef.current = null;
      return;
    }
    if (!stream || (!shouldScrollToLatestRef.current && !isNearLatestRef.current)) return;
    stream.scrollTop = stream.scrollHeight;
    shouldScrollToLatestRef.current = false;
    isNearLatestRef.current = true;
  }, [selectedId, messages.length, latestMessageId]);

  async function loadOlderMessages() {
    const stream = messageStreamRef.current;
    if (!stream) return;
    historyScrollAnchorRef.current = { height: stream.scrollHeight, top: stream.scrollTop };
    try {
      await loadOlder();
    } catch (error) {
      historyScrollAnchorRef.current = null;
      props.onError(error);
    }
  }

  function trackMessageScroll() {
    const stream = messageStreamRef.current;
    if (!stream) return;
    isNearLatestRef.current = stream.scrollHeight - stream.scrollTop - stream.clientHeight <= 80;
  }

  async function openDirect(event: FormEvent) {
    event.preventDefault();
    if (!targetAgentId) return;
    setBusy(true);
    try {
      const response = await api<{ conversation: Conversation }>(`/api/v1/companies/${props.consoleData.company.id}/conversations/direct`, {
        method: "POST",
        body: JSON.stringify({ target_agent_id: targetAgentId }),
      }, props.token);
      setSelectedId(response.conversation.preview.id);
      setShowNewDirect(false);
      setTargetAgentId("");
      await props.onChanged();
      props.onNotice("Human 私聊已建立，消息会在 Agent 下次 Trigger 时被发现。");
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  function addPendingFiles(files: File[]) {
    const availableSlots = Math.max(0, 20 - pendingFiles.length);
    const accepted: PendingMessageFile[] = [];
    for (const file of files.slice(0, availableSlots)) {
      if (file.size > 20 * 1024 * 1024) {
        props.onError(new Error(`${file.name} 超过单文件 20 MiB 限制`));
        continue;
      }
      accepted.push({
        id: crypto.randomUUID(),
        file,
        relativePath: file.webkitRelativePath || "",
      });
    }
    if (files.length > availableSlots) props.onError(new Error("每条消息最多添加 20 个附件或文件夹引用"));
    setPendingFiles((current) => [...current, ...accepted]);
  }

  function handleMessagePaste(event: ClipboardEvent<HTMLTextAreaElement>) {
    const files = Array.from(event.clipboardData.files);
    if (!files.length) return;
    event.preventDefault();
    addPendingFiles(files);
  }

  async function sendMessage(event: FormEvent) {
    event.preventDefault();
    if (!selectedId || (!draft.trim() && !pendingFiles.length)) return;
    setBusy(true);
    try {
      const effectiveMentionAll = selectedIsGroup && mentionAll && draft.includes("@所有人");
      const effectiveMentionedAgentIds = effectiveMentionAll ? [] : mentionedAgentIds.filter((agentId) => {
        const agent = agentDirectory.get(agentId);
        if (!agent) return false;
        return draft.includes(`@${agent.agent_profile.display_name}`)
          || draft.includes(`@${agent.agent_profile.handle.replace(/^@/, "")}`);
      });
      if (pendingFiles.length) {
        const form = new FormData();
        form.append("content", draft);
        form.append("mentioned_agent_ids", JSON.stringify(effectiveMentionedAgentIds));
        form.append("mention_all", String(effectiveMentionAll));
        form.append("relative_paths", JSON.stringify(pendingFiles.map((item) => item.relativePath)));
        pendingFiles.forEach((item) => form.append("file", item.file, item.file.name));
        await api(`/api/v1/companies/${props.consoleData.company.id}/conversations/${selectedId}/messages/with-attachments`, {
          method: "POST",
          body: form,
        }, props.token);
      } else {
        await api(`/api/v1/companies/${props.consoleData.company.id}/conversations/${selectedId}/messages`, {
          method: "POST",
          body: JSON.stringify({
            content: draft,
            mentioned_agent_ids: effectiveMentionedAgentIds,
            mention_all: effectiveMentionAll,
          }),
        }, props.token);
      }
      setDraft("");
      setPendingFiles([]);
      setMentionedAgentIds([]);
      setMentionAll(false);
      setMentionQuery(null);
      shouldScrollToLatestRef.current = true;
      await Promise.all([refreshLatest(), props.onChanged()]);
      props.onNotice(selected?.preview.conversation_type === "direct"
        ? "消息已发送，目标 Agent 正在被即时唤醒。"
        : effectiveMentionAll || !effectiveMentionedAgentIds.length
          ? "群消息已发送，群内 Agent 正在被即时唤醒。"
          : `群消息已发送，仅即时唤醒 ${effectiveMentionedAgentIds.length} 个被 @ 的 Agent。`);
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  function updateDraft(value: string) {
    setDraft(value);
    if (!selectedIsGroup) {
      setMentionQuery(null);
      return;
    }
    const match = value.match(/(?:^|\s)@([^\s@]*)$/u);
    setMentionQuery(match ? match[1] : null);
    if (!value.includes("@所有人")) setMentionAll(false);
    setMentionedAgentIds((current) => current.filter((agentId) => {
      const agent = agentDirectory.get(agentId);
      return Boolean(agent && (
        value.includes(`@${agent.agent_profile.display_name}`)
        || value.includes(`@${agent.agent_profile.handle.replace(/^@/, "")}`)
      ));
    }));
  }

  function insertMention(agent?: CompanyAgent) {
    const mentionText = agent ? agent.agent_profile.display_name : "所有人";
    const match = draft.match(/(?:^|\s)@([^\s@]*)$/u);
    const mentionStart = match ? (match.index ?? 0) + match[0].lastIndexOf("@") : -1;
    const nextDraft = mentionStart >= 0
      ? `${draft.slice(0, mentionStart)}@${mentionText} `
      : `${draft}${draft && !draft.endsWith(" ") ? " " : ""}@${mentionText} `;
    setDraft(nextDraft);
    setMentionQuery(null);
    if (agent) {
      setMentionAll(false);
      setMentionedAgentIds((current) => current.includes(agent.agent_profile.id)
        ? current
        : [...current, agent.agent_profile.id]);
    } else {
      setMentionAll(true);
      setMentionedAgentIds([]);
    }
  }

  function removeMention(agentId?: string) {
    if (!agentId) {
      setMentionAll(false);
      setDraft((current) => current.replace(/@所有人\s*/gu, ""));
      return;
    }
    const agent = agentDirectory.get(agentId);
    setMentionedAgentIds((current) => current.filter((id) => id !== agentId));
    if (!agent) return;
    const tokens = [agent.agent_profile.display_name, agent.agent_profile.handle.replace(/^@/, "")]
      .map(escapeRegExp)
      .join("|");
    setDraft((current) => current.replace(new RegExp(`@(?:${tokens})\\s*`, "gu"), ""));
  }

  return (
    <>
      <section className={`message-console ${showMemberDetails && selected ? "members-open" : ""}`}>
        <div className="conversation-list">
          <div className="conversation-list-head">
            <strong>会话</strong>
            {canSend ? <button className="new-direct-button" type="button" onClick={() => setShowNewDirect(true)}><Icon name="plus" /> 新建私聊</button> : null}
          </div>
          <label className="conversation-search"><Icon name="search" /><input value={conversationQuery} onChange={(event) => setConversationQuery(event.target.value)} placeholder="搜索会话" /></label>
          {conversationPagination.pageItems.map((conversation) => (
            <button key={conversation.preview.id} className={selectedId === conversation.preview.id ? "active" : ""} onClick={() => setSelectedId(conversation.preview.id)}>
              <span className="conversation-icon"><Icon name={conversation.preview.conversation_type === "group" ? "group" : "message"} /></span>
              <span><strong>{conversationDisplayTitle(conversation, agentNames)}</strong><small>{conversation.preview.last_message_preview ?? "暂无消息"}</small></span>
            </button>
          ))}
          <Pagination {...conversationPagination} onPageChange={conversationPagination.setPage} compact />
          {!props.consoleData.conversations.length ? <div className="conversation-list-empty">还没有会话，可以先与一个 Agent 建立私聊。</div> : null}
        </div>
        <div className="message-panel">
          <div className="message-head">
            <div><strong>{conversationDisplayTitle(selected, agentNames) || "选择一个会话"}</strong></div>
            <div className="message-head-actions">
              {selected && selectedConversationAgents.length ? <button className={`group-members-button ${showMemberDetails ? "active" : ""}`} type="button" aria-expanded={showMemberDetails} onClick={() => setShowMemberDetails((current) => !current)}><Icon name={selectedIsGroup ? "group" : "message"} /> {selectedIsGroup ? "群成员" : "运行详情"} <span>{selectedConversationAgents.length}</span></button> : null}
              {selected ? <span className="pill neutral">{formatConversationContext(selected.context.context_type)}</span> : null}
              {selectedProjectPaused ? <span className="pill paused">项目已暂停</span> : null}
            </div>
          </div>
          <div className="message-stream" ref={messageStreamRef} onScroll={trackMessageScroll}>
            <MessageHistoryControl visible={Boolean(selected && hasOlderMessages)} loading={loadingOlder} onLoad={() => void loadOlderMessages()} />
            {loadingMessages && !messages.length ? <div className="empty-messages">正在加载消息…</div> : messages.length ? messages.map((message) => {
              const isHuman = message.sender_human_user_id !== null;
              const senderName = isHuman ? props.humanUser.display_name : agentNames.get(message.sender_agent_id ?? "") ?? "Unknown Agent";
              return <article className={`message-item ${isHuman ? "human" : ""}`} key={message.id}>
                <span className="agent-avatar small">{senderName.slice(0, 1)}</span>
                <div className="message-body">
                  <header className="message-meta"><strong>{senderName}{isHuman ? " · Human" : ""}</strong><time>{formatTime(message.created_at)}</time></header>
                  {message.content ? <div className="message-bubble">{renderMessageContent(message.content, selectedConversationAgents)}</div> : null}
                  <MessageAttachments message={message} token={props.token} onError={props.onError} />
                </div>
              </article>;
            }) : <div className="empty-messages">{selected ? "这段会话还没有消息。" : "请选择一个会话。"}</div>}
          </div>
          {canSend && !selectedProjectPaused ? (
            <form className="message-composer" onSubmit={sendMessage}>
              <div className="message-input-shell">
                {mentionQuery !== null ? <div className="mention-menu">
                  {showMentionAllCandidate ? <button type="button" onMouseDown={(event) => { event.preventDefault(); insertMention(); }}><span className="mention-avatar all">@</span><span><strong>所有人</strong><small>唤醒群内全部 Agent</small></span></button> : null}
                  {mentionCandidates.map((agent) => <button key={agent.agent_profile.id} type="button" onMouseDown={(event) => { event.preventDefault(); insertMention(agent); }}><span className="mention-avatar">{agent.agent_profile.display_name.slice(0, 1)}</span><span><strong>{agent.agent_profile.display_name}</strong><small>@{agent.agent_profile.handle.replace(/^@/, "")} · {agent.membership.job_title || "Agent"}</small></span></button>)}
                  {!mentionCandidates.length && !showMentionAllCandidate ? <div className="mention-empty">没有匹配的群成员</div> : null}
                </div> : null}
                {pendingFiles.length ? <div className="pending-attachments">
                  {pendingFiles.map((item) => <div className="pending-attachment" key={item.id}><span className="attachment-kind-icon"><Icon name={item.file.type.startsWith("image/") ? "image" : "paperclip"} /></span><span><strong>{item.relativePath || item.file.name}</strong><small>{formatByteSize(item.file.size)}</small></span><button type="button" onClick={() => setPendingFiles((current) => current.filter((file) => file.id !== item.id))}><Icon name="close" /></button></div>)}
                </div> : null}
                <textarea value={draft} onChange={(event) => updateDraft(event.target.value)} onPaste={handleMessagePaste} onKeyDown={(event) => {
                  if (mentionQuery === null) return;
                  if (event.key === "Escape") { event.preventDefault(); setMentionQuery(null); }
                  if ((event.key === "Enter" || event.key === "Tab") && !event.shiftKey) {
                    const candidate = mentionCandidates[0];
                    if (candidate || showMentionAllCandidate) {
                      event.preventDefault();
                      insertMention(candidate);
                    }
                  }
                }} placeholder={selectedIsGroup ? "输入 @ 选择要立即唤醒的 Agent；也可直接粘贴截图…" : "给 Agent 分配任务、补充信息；也可直接粘贴截图…"} disabled={!selected || busy} />
                <div className="attachment-actions">
                  <button type="button" disabled={!selected || busy} onClick={() => fileInputRef.current?.click()}><Icon name="paperclip" /> 选择文件</button>
                </div>
                <input ref={fileInputRef} className="hidden-file-input" type="file" multiple onChange={(event) => { addPendingFiles(Array.from(event.target.files ?? [])); event.target.value = ""; }} />
                {selectedIsGroup && (mentionAll || mentionedAgentIds.length) ? <div className="mention-chips">
                  {mentionAll ? <button type="button" onClick={() => removeMention()}><span>@所有人</span><Icon name="close" /></button> : mentionedAgentIds.map((agentId) => {
                    const agent = agentDirectory.get(agentId);
                    return agent ? <button type="button" key={agentId} onClick={() => removeMention(agentId)}><span>@{agent.agent_profile.display_name}</span><Icon name="close" /></button> : null;
                  })}
                </div> : null}
              </div>
              <button className="button primary" disabled={!selected || (!draft.trim() && !pendingFiles.length) || busy}>{busy ? "发送中…" : "发送消息"}</button>
            </form>
          ) : <div className={`observer-note ${selectedProjectPaused ? "paused" : ""}`}><Icon name={selectedProjectPaused ? "pause" : "eye"} /> {selectedProjectPaused ? "项目已暂停，恢复后可发送消息" : "当前账号仅可查看消息"}</div>}
        </div>
        {showMemberDetails && selected && (selectedIsGroup || selectedIsDirect) ? (
          <GroupMembersDrawer
            companyId={props.consoleData.company.id}
            conversationTitle={conversationDisplayTitle(selected, agentNames)}
            humanName={props.humanUser.display_name}
            agents={selectedConversationAgents}
            project={selectedProject ?? null}
            mode={selectedIsGroup ? "group" : "direct"}
            token={props.token}
            realtimeEvent={props.realtimeEvent}
            onClose={() => setShowMemberDetails(false)}
          />
        ) : null}
      </section>
      {showNewDirect ? (
        <Dialog title="新建 Human 私聊" description="选择一个活跃 Agent。重复选择同一个 Agent 会打开原有私聊。" onClose={() => setShowNewDirect(false)}>
          <form className="stack-form" onSubmit={openDirect}>
            <Field label="目标 Agent"><select value={targetAgentId} onChange={(event) => setTargetAgentId(event.target.value)} required><option value="">请选择 Agent</option>{activeAgents.map((agent) => <option key={agent.agent_profile.id} value={agent.agent_profile.id}>{agent.agent_profile.display_name} · {agent.membership.job_title}</option>)}</select></Field>
            {!activeAgents.length ? <div className="git-security-note">当前没有可接收消息的活跃 Agent。</div> : null}
            <button className="button primary wide" disabled={!targetAgentId || busy}>{busy ? "正在建立…" : "建立私聊"}</button>
          </form>
        </Dialog>
      ) : null}
    </>
  );
}


function Dialog(props: { title: string; description?: string; onClose: () => void; children: ReactNode }) {
  return <div className="dialog-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) props.onClose(); }}><div className="dialog" role="dialog" aria-modal="true"><div className="dialog-head"><div><h2>{props.title}</h2>{props.description ? <p>{props.description}</p> : null}</div><button className="icon-button" onClick={props.onClose}><Icon name="close" /></button></div>{props.children}</div></div>;
}

function StatusBadge({ value }: { value: string }) {
  const label = { active: "可连接", connected: "已连接", not_connected: "待连接", awaiting_activation: "待激活", provisioning: "待激活", suspended: "已暂停", terminated: "已裁撤", key_revoked: "Key 已撤销", key_expired: "Key 已过期", no_key: "无 Key", running: "运行中", succeeded: "成功", failed: "失败", timed_out: "超时", cancelled: "已取消", lease_lost: "租约丢失", approved: "已批准", rejected: "已拒绝" }[value] ?? value;
  return <span className={`status-badge ${value}`}><span className="status-dot" />{label}</span>;
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function renderMessageContent(content: string, agents: CompanyAgent[]): ReactNode {
  const mentionTokens = Array.from(new Set([
    "所有人",
    ...agents.flatMap((agent) => [
      agent.agent_profile.display_name,
      agent.agent_profile.handle.replace(/^@/, ""),
    ]),
  ])).filter(Boolean).sort((left, right) => right.length - left.length);
  const inlinePattern = new RegExp(`(\`[^\`]+\`|https?:\\/\\/[^\\s]+${mentionTokens.length ? `|@(?:${mentionTokens.map(escapeRegExp).join("|")})` : ""})`, "gu");
  const renderInline = (value: string, keyPrefix: string) => value.split(inlinePattern).map((part, index) => {
    if (part.startsWith("@")) return <mark className="message-mention" key={`${keyPrefix}-${index}`}>{part}</mark>;
    if (part.startsWith("`") && part.endsWith("`")) return <code key={`${keyPrefix}-${index}`}>{part.slice(1, -1)}</code>;
    if (/^https?:\/\//u.test(part)) return <a href={part} target="_blank" rel="noreferrer" key={`${keyPrefix}-${index}`}>{part}</a>;
    return part;
  });
  return <div className="message-rich-text">{content.trim().split(/\n{2,}/u).map((block, blockIndex) => {
    const lines = block.split("\n").filter((line) => line.trim());
    const numbered = lines.length > 1 && lines.every((line) => /^\s*\d+[.)]\s+/u.test(line));
    const bulleted = lines.length > 1 && lines.every((line) => /^\s*[-*]\s+/u.test(line));
    if (numbered || bulleted) {
      const List = numbered ? "ol" : "ul";
      return <List key={`block-${blockIndex}`}>{lines.map((line, lineIndex) => <li key={`line-${lineIndex}`}>{renderInline(line.replace(numbered ? /^\s*\d+[.)]\s+/u : /^\s*[-*]\s+/u, ""), `block-${blockIndex}-line-${lineIndex}`)}</li>)}</List>;
    }
    return <p key={`block-${blockIndex}`}>{lines.map((line, lineIndex) => <Fragment key={`line-${lineIndex}`}>{renderInline(line, `block-${blockIndex}-line-${lineIndex}`)}{lineIndex < lines.length - 1 ? <br /> : null}</Fragment>)}</p>;
  })}</div>;
}

function formatTime(value: string) {
  return new Date(value).toLocaleString("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" });
}

function formatByteSize(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KiB`;
  return `${(value / 1024 / 1024).toFixed(1)} MiB`;
}

function conversationDisplayTitle(conversation: Conversation | undefined, agentNames: Map<string, string>) {
  if (!conversation) return "";
  const isDirect = conversation.preview.conversation_type === "direct" || conversation.context.context_type === "company_direct";
  if (!isDirect) return conversation.preview.title;
  const memberNames = conversation.member_agent_ids
    .map((agentId) => agentNames.get(agentId))
    .filter((name): name is string => Boolean(name));
  return memberNames.length >= 2 ? memberNames.join(" ↔ ") : conversation.preview.title;
}

function formatConversationContext(value?: string) {
  return { company_all: "公司全员群", company_group: "公司群", project_group: "项目群", company_direct: "公司私聊" }[value ?? ""] ?? value ?? "公司会话";
}

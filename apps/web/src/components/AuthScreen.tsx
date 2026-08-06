import { FormEvent, useState } from "react";

import { api } from "../api/client";
import { useUiLanguage } from "../i18n/uiLanguage";
import type { HumanUser, RuntimeConfig, Session } from "../types/appShell";
import { Field, FlowStep, Icon } from "./ui";

export function AuthScreen(props: {
  runtimeConfig: RuntimeConfig | null;
  busy: boolean;
  error: string;
  setBusy: (value: boolean) => void;
  setError: (value: string) => void;
  onAuthenticated: (session: Session) => void;
}) {
  const { language, setLanguage } = useUiLanguage();
  const [mode, setMode] = useState<"login" | "register">("login");
  const [email, setEmail] = useState("owner@example.com");
  const [displayName, setDisplayName] = useState("Owner");
  const [password, setPassword] = useState("password123");

  async function submit(event: FormEvent) {
    event.preventDefault();
    props.setBusy(true);
    props.setError("");
    try {
      const response = await api<{ user: HumanUser; session_token: string }>(
        mode === "login" ? "/api/v1/auth/login" : "/api/v1/auth/register",
        {
          method: "POST",
          body: JSON.stringify(
            mode === "login" ? { email, password } : { email, display_name: displayName, password },
          ),
        },
      );
      props.onAuthenticated({ token: response.session_token, user: response.user });
    } catch (requestError) {
      props.setError(requestError instanceof Error ? requestError.message : "登录失败");
    } finally {
      props.setBusy(false);
    }
  }

  async function devLogin() {
    props.setBusy(true);
    props.setError("");
    try {
      const response = await api<{ user: HumanUser; session_token: string }>("/api/v1/dev/login", {
        method: "POST",
        body: JSON.stringify({ email, display_name: displayName }),
      });
      props.onAuthenticated({ token: response.session_token, user: response.user });
    } catch (requestError) {
      props.setError(requestError instanceof Error ? requestError.message : "开发登录失败");
    } finally {
      props.setBusy(false);
    }
  }

  return (
    <div className="auth-shell">
      <section className="auth-story">
        <div className="auth-story-topline">
          <span className="auth-brand"><span className="brand-mark"><Icon name="network" /></span><strong>Relay</strong></span>
          <span className="auth-system-state"><i /> CONTROL PLANE ONLINE</span>
        </div>
        <div className="auth-story-copy">
          <span className="eyebrow">LOCAL AGENT OPERATIONS</span>
          <h1>让每一次唤醒，<br />都接得住未完的远方。</h1>
          <p>Relay 把本地 Codex 的消息、任务、记忆与项目编织成一条持续协作的航线。Human 掌舵，Agent 接力，让每一段工作都有来路，也最终沉淀为清晰、可验证、可持续演进的成果。</p>
          <div className="auth-signal-strip" aria-label="Relay 能力状态">
            <span><i /> LOCAL CODEX</span>
            <span><i /> MCP CONNECTED</span>
            <span><i /> HUMAN IN CONTROL</span>
          </div>
        </div>

        <div className="auth-network-stage" aria-hidden="true">
          <div className="auth-orbit auth-orbit-one" />
          <div className="auth-orbit auth-orbit-two" />
          <svg className="auth-network-lines" viewBox="0 0 620 330" preserveAspectRatio="none">
            <path d="M310 165 112 78M310 165 500 62M310 165 530 245M310 165 130 260" />
            <path className="auth-network-line-active" d="M112 78 310 165 500 62M130 260 310 165 530 245" />
          </svg>
          <div className="auth-core-node"><span className="auth-core-mark"><Icon name="network" /></span><small>RELAY CORE</small><strong>Human Orchestrated</strong><em>05 agents connected</em></div>
          <div className="auth-agent-node auth-agent-node-a"><span><Icon name="terminal" /></span><small>ENGINEERING</small><strong>Codex / fixed session</strong><em>RUNNING</em></div>
          <div className="auth-agent-node auth-agent-node-b"><span><Icon name="tasks" /></span><small>PRODUCT</small><strong>Task inbox updated</strong><em>READY</em></div>
          <div className="auth-agent-node auth-agent-node-c"><span><Icon name="git" /></span><small>DELIVERY</small><strong>Branch verified</strong><em>SYNCED</em></div>
          <div className="auth-agent-node auth-agent-node-d"><span><Icon name="message" /></span><small>COLLABORATION</small><strong>Message received</strong><em>TRIGGERED</em></div>
          <div className="auth-event-chip auth-event-chip-one"><i /> message.new</div>
          <div className="auth-event-chip auth-event-chip-two"><i /> codex.resume</div>
          <div className="auth-event-chip auth-event-chip-three"><i /> git.push</div>
        </div>

        <div className="auth-flow">
          <FlowStep index="01" title="接入本地 Codex" detail="保留你的模型、工具与宿主机环境" />
          <FlowStep index="02" title="按事件精准唤醒" detail="消息、任务与定时触发同一会话" />
          <FlowStep index="03" title="持续协作交付" detail="任务、记忆、审批与 Git 全程可见" />
        </div>
      </section>
      <section className="auth-panel">
        <div className="auth-language-switch" role="group" aria-label="Interface language">
          <button className={language === "zh-CN" ? "active" : ""} type="button" onClick={() => setLanguage("zh-CN")}>中文</button>
          <button className={language === "en" ? "active" : ""} type="button" onClick={() => setLanguage("en")}>English</button>
        </div>
        <form className="auth-card" onSubmit={submit} autoComplete="on">
          <div className="auth-card-brand">
            <span className="brand-mark small"><Icon name="network" /></span>
            <span><strong>Relay Control Plane</strong><small>Local-first agent collaboration</small></span>
          </div>
          <div className="segmented">
            <button type="button" className={mode === "login" ? "active" : ""} onClick={() => setMode("login")}>登录</button>
            <button type="button" className={mode === "register" ? "active" : ""} onClick={() => setMode("register")}>注册</button>
          </div>
          <div className="auth-heading">
            <span className="eyebrow">HUMAN CONSOLE</span>
            <h2>{mode === "login" ? "进入你的 Agent 公司" : "创建 Human 管理账号"}</h2>
            <p>{mode === "login" ? "查看 Agent 正在做什么，并在关键节点给出指令与审批。" : "Human 负责目标、组织和授权；实际工作由本地 Agent 持续完成。"}</p>
          </div>
          {mode === "register" ? (
            <Field label="你的名字">
              <input value={displayName} onChange={(event) => setDisplayName(event.target.value)} autoComplete="name" required />
            </Field>
          ) : null}
          <Field label="邮箱">
            <input type="email" value={email} onChange={(event) => setEmail(event.target.value)} autoComplete="email" required />
          </Field>
          <Field label="密码">
            <input type="password" value={password} onChange={(event) => setPassword(event.target.value)} autoComplete={mode === "login" ? "current-password" : "new-password"} minLength={8} required />
          </Field>
          {props.error ? <div className="inline-error">{props.error}</div> : null}
          <button className="button primary wide" disabled={props.busy}>
            {props.busy ? "正在建立安全会话…" : mode === "login" ? "进入 Relay 控制台" : "创建账号"}
          </button>
          {props.runtimeConfig?.dev_endpoints_enabled ? (
            <button className="button ghost wide" type="button" onClick={() => void devLogin()} disabled={props.busy}>
              开发环境快速进入
            </button>
          ) : null}
          <div className="auth-local-note"><Icon name="shield" /><span><strong>本地优先</strong><small>模型调用、代码与工作目录仍留在你的宿主机。</small></span></div>
        </form>
      </section>
    </div>
  );
}

import { useState } from "react";
import type { CompanyRealtimeEvent } from "../../api/types";
import { CodexCliSettingsView } from "../../components/CodexCliSettingsView";
import { Icon } from "../../components/ui";
import type { CompanyConsole } from "../../types/platform";
import { CodexCliAuthView } from "./auth";
import { CodexMcpView } from "./mcp";
import { CodexPluginsView } from "./plugins";
import { CodexRunnersView } from "./runners";

export function CodexControlCenter(props: {
  consoleData: CompanyConsole;
  token: string;
  realtimeEvent: CompanyRealtimeEvent | null;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const [tab, setTab] = useState<"runners" | "settings" | "mcp" | "plugins" | "environment">("runners");

  return (
    <div className="control-center codex-control-center">
      <nav className="control-center-tabs" role="tablist" aria-label="Codex 控制台">
        <button
          className={tab === "runners" ? "active" : ""}
          type="button"
          role="tab"
          aria-selected={tab === "runners"}
          onClick={() => setTab("runners")}
        >
          <span className="control-center-tab-icon"><Icon name="terminal" /></span>
          <span><strong>运行器</strong><small>配置与会话</small></span>
        </button>
        <button
          className={tab === "mcp" ? "active" : ""}
          type="button"
          role="tab"
          aria-selected={tab === "mcp"}
          onClick={() => setTab("mcp")}
        >
          <span className="control-center-tab-icon"><Icon name="network" /></span>
          <span><strong>MCP</strong><small>服务与连接</small></span>
        </button>
        <button
          className={tab === "plugins" ? "active" : ""}
          type="button"
          role="tab"
          aria-selected={tab === "plugins"}
          onClick={() => setTab("plugins")}
        >
          <span className="control-center-tab-icon"><Icon name="plugin" /></span>
          <span><strong>插件</strong><small>CLI 能力目录</small></span>
        </button>
        <button
          className={tab === "environment" ? "active" : ""}
          type="button"
          role="tab"
          aria-selected={tab === "environment"}
          onClick={() => setTab("environment")}
        >
          <span className="control-center-tab-icon"><Icon name="key" /></span>
          <span><strong>CLI 与认证</strong><small>安装与账号环境</small></span>
        </button>
        <button
          className={tab === "settings" ? "active" : ""}
          type="button"
          role="tab"
          aria-selected={tab === "settings"}
          onClick={() => setTab("settings")}
        >
          <span className="control-center-tab-icon"><Icon name="settings" /></span>
          <span><strong>CLI 设置</strong><small>高级默认项</small></span>
        </button>
      </nav>

      <div className="control-center-panel" role="tabpanel">
        {tab === "runners" ? (
          <CodexRunnersView
            consoleData={props.consoleData}
            token={props.token}
            realtimeEvent={props.realtimeEvent}
            onError={props.onError}
            onNotice={props.onNotice}
          />
        ) : null}
        {tab === "plugins" ? (
          <CodexPluginsView
            companyId={props.consoleData.company.id}
            token={props.token}
            realtimeEvent={props.realtimeEvent}
            onError={props.onError}
            onNotice={props.onNotice}
          />
        ) : null}
        {tab === "settings" ? (
          <CodexCliSettingsView
            companyId={props.consoleData.company.id}
            token={props.token}
            onError={props.onError}
            onNotice={props.onNotice}
          />
        ) : null}
        {tab === "mcp" ? (
          <CodexMcpView
            companyId={props.consoleData.company.id}
            token={props.token}
            onError={props.onError}
            onNotice={props.onNotice}
          />
        ) : null}
        {tab === "environment" ? (
          <CodexCliAuthView
            companyId={props.consoleData.company.id}
            token={props.token}
            onError={props.onError}
            onNotice={props.onNotice}
          />
        ) : null}
      </div>
    </div>
  );
}


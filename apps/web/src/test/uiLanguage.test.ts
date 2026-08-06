import { describe, expect, it } from "vitest";

import { translateUiText } from "../i18n/uiLanguage";

describe("UI language translation", () => {
  it("translates fixed interface labels to English", () => {
    expect(translateUiText("Codex 控制台", "en")).toBe("Codex Console");
    expect(translateUiText("CLI 与认证", "en")).toBe("CLI & Authentication");
    expect(translateUiText("发送消息", "en")).toBe("Send Message");
  });

  it("translates dynamic interface counters while preserving whitespace", () => {
    expect(translateUiText("  3 个已连接，7 个待连接  ", "en"))
      .toBe("  3 connected, 7 awaiting connection  ");
    expect(translateUiText("批量激活 4 个", "en")).toBe("Activate 4");
    expect(translateUiText("3600 秒", "en")).toBe("3600 seconds");
    expect(translateUiText("2 小时 5 分", "en")).toBe("2h 5m");
  });

  it("translates system suffixes without translating user-defined names", () => {
    expect(translateUiText("（通用默认）", "en")).toBe(" (Company Default)");
    expect(translateUiText("规划模式", "en")).toBe("规划模式");
  });

  it("does not change content when Chinese is selected", () => {
    expect(translateUiText("项目中心", "zh-CN")).toBe("项目中心");
  });
});

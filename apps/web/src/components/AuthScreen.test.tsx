import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import { AuthScreen } from "./AuthScreen";

vi.mock("../api/client", () => ({ api: vi.fn() }));

const mockedApi = vi.mocked(api);

describe("AuthScreen", () => {
  it("submits registration through the shared API client", async () => {
    const onAuthenticated = vi.fn();
    const setBusy = vi.fn();
    const setError = vi.fn();
    mockedApi.mockResolvedValueOnce({
      session_token: "session-token",
      user: { id: "human-1", email: "owner@example.com", display_name: "Owner" },
    });

    render(
      <AuthScreen
        runtimeConfig={{ dev_endpoints_enabled: false, email_verification_required: false, project_types: [] }}
        busy={false}
        error=""
        setBusy={setBusy}
        setError={setError}
        onAuthenticated={onAuthenticated}
      />,
    );

    expect(screen.getByLabelText("邮箱")).toHaveAttribute("autocomplete", "email");
    expect(screen.getByLabelText("密码")).toHaveAttribute("autocomplete", "current-password");

    fireEvent.click(screen.getByRole("button", { name: "注册" }));
    expect(screen.getByLabelText("你的名字")).toHaveAttribute("autocomplete", "name");
    expect(screen.getByLabelText("密码")).toHaveAttribute("autocomplete", "new-password");
    fireEvent.click(screen.getByRole("button", { name: "创建账号" }));

    await waitFor(() => expect(onAuthenticated).toHaveBeenCalledWith({
      token: "session-token",
      user: { id: "human-1", email: "owner@example.com", display_name: "Owner" },
    }));
    expect(mockedApi).toHaveBeenCalledWith("/api/v1/auth/register", {
      method: "POST",
      body: JSON.stringify({
        email: "owner@example.com",
        display_name: "Owner",
        password: "password123",
      }),
    });
    expect(setBusy).toHaveBeenNthCalledWith(1, true);
    expect(setBusy).toHaveBeenLastCalledWith(false);
  });
});

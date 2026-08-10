import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CompanyProject } from "../../types/platform";
import { ProjectRepositoryBrowser } from "./repository";

const project = {
  project: { id: "project-1", name: "Demo" },
  git: {
    remote_url: "https://harness.example.test/git/space/demo.git",
    default_branch: "main",
    git_host: "harness.example.test",
    push_enabled: true,
    branch_prefix: "relay/",
    auth_configured: true,
  },
} as CompanyProject;

afterEach(() => vi.restoreAllMocks());

describe("ProjectRepositoryBrowser", () => {
  it("browses Harness directories, previews highlighted files, and changes branch", async () => {
    const requests: string[] = [];
    vi.stubGlobal("fetch", vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      requests.push(url);
      if (url.endsWith("/repository/refs")) {
        return jsonResponse({
          refs: [
            { name: "main", full_name: "main", commit: "1111111111111111111111111111111111111111", kind: "branch", is_default: true },
            { name: "release", full_name: "release", commit: "2222222222222222222222222222222222222222", kind: "branch", is_default: false },
          ],
          default_ref: "main",
          refreshed_at: "2026-08-07T00:00:00Z",
          source: "harness_api",
        });
      }
      if (url.includes("/repository/tree?")) {
        const query = new URL(url).searchParams;
        const path = query.get("path") ?? "";
        return jsonResponse({
          reference: query.get("ref"),
          commit: query.get("ref") === "release" ? "2222222222222222222222222222222222222222" : "1111111111111111111111111111111111111111",
          path,
          entries: path === "src"
            ? [{ name: "main.rs", path: "src/main.rs", kind: "file", size: 13, mode: "100644" }]
            : [{ name: "src", path: "src", kind: "directory", size: null, mode: "" }],
          page: 1,
          per_page: 100,
          total: 1,
          total_pages: 1,
        });
      }
      if (url.includes("/repository/file?")) {
        return jsonResponse({
          reference: "main",
          commit: "1111111111111111111111111111111111111111",
          path: "src/main.rs",
          name: "main.rs",
          size: 13,
          line_count: 1,
          binary: false,
          content: "fn main() {}\n",
          language: "rs",
        });
      }
      return jsonResponse({ message: "unexpected request" }, 500);
    }));

    const { container } = render(<ProjectRepositoryBrowser companyId="company-1" project={project} token="token" onError={() => undefined} />);

    fireEvent.click(await screen.findByRole("button", { name: /src/u }));
    fireEvent.click(await screen.findByRole("button", { name: /main\.rs/u }));
    await waitFor(() => expect(container.querySelector("code.hljs")?.textContent).toContain("fn main() {}"));
    expect(screen.getByText("rust")).toBeInTheDocument();

    fireEvent.change(screen.getByRole("combobox", { name: "浏览分支" }), { target: { value: "release" } });
    await waitFor(() => expect(requests.some((url) => url.includes("ref=release") && url.includes("path="))).toBe(true));
  });

  it("requests the next server-side directory page", async () => {
    const requestedPages: string[] = [];
    vi.stubGlobal("fetch", vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.endsWith("/repository/refs")) {
        return jsonResponse({
          refs: [{ name: "main", full_name: "main", commit: "1".repeat(40), kind: "branch", is_default: true }],
          default_ref: "main",
          refreshed_at: "2026-08-07T00:00:00Z",
          source: "harness_api",
        });
      }
      if (url.includes("/repository/tree?")) {
        const page = new URL(url).searchParams.get("page") ?? "1";
        requestedPages.push(page);
        return jsonResponse({
          reference: "main",
          commit: "1".repeat(40),
          path: "",
          entries: [{ name: `page-${page}.txt`, path: `page-${page}.txt`, kind: "file", size: 1, mode: "100644" }],
          page: Number(page),
          per_page: 100,
          total: 101,
          total_pages: 2,
        });
      }
      return jsonResponse({ message: "unexpected request" }, 500);
    }));

    render(<ProjectRepositoryBrowser companyId="company-1" project={project} token="token" onError={() => undefined} />);

    await screen.findByText("page-1.txt");
    fireEvent.click(screen.getByRole("button", { name: "下一页" }));
    await screen.findByText("page-2.txt");
    expect(requestedPages).toContain("2");
  });

  it("renders SVG files visually by default and keeps source view available", async () => {
    vi.stubGlobal("fetch", vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.endsWith("/repository/refs")) {
        return jsonResponse({
          refs: [{ name: "main", full_name: "main", commit: "1".repeat(40), kind: "branch", is_default: true }],
          default_ref: "main",
          refreshed_at: "2026-08-09T00:00:00Z",
          source: "harness_api",
        });
      }
      if (url.includes("/repository/tree?")) {
        return jsonResponse({
          reference: "main",
          commit: "1".repeat(40),
          path: "",
          entries: [{ name: "diagram.svg", path: "diagram.svg", kind: "file", size: 114, mode: "100644" }],
          page: 1,
          per_page: 100,
          total: 1,
          total_pages: 1,
        });
      }
      if (url.includes("/repository/file?")) {
        return jsonResponse({
          reference: "main",
          commit: "1".repeat(40),
          path: "diagram.svg",
          name: "diagram.svg",
          size: 114,
          line_count: 1,
          binary: false,
          content: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50"><text x="10" y="30">流程图</text></svg>',
          language: "svg",
        });
      }
      return jsonResponse({ message: "unexpected request" }, 500);
    }));

    const { container } = render(<ProjectRepositoryBrowser companyId="company-1" project={project} token="token" onError={() => undefined} />);

    fireEvent.click(await screen.findByRole("button", { name: /diagram\.svg/u }));
    const preview = await screen.findByRole("img", { name: "diagram.svg 预览" });
    expect(preview.getAttribute("src")).toMatch(/^data:image\/svg\+xml;charset=utf-8,/u);
    expect(screen.getByRole("button", { name: "预览" })).toHaveAttribute("aria-pressed", "true");

    fireEvent.click(screen.getByRole("button", { name: "源码" }));
    await waitFor(() => expect(container.querySelector("code.hljs")?.textContent).toContain("<svg"));
    expect(screen.getByRole("button", { name: "源码" })).toHaveAttribute("aria-pressed", "true");
  });
});

function jsonResponse(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

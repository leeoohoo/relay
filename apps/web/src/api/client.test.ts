import { afterEach, describe, expect, it, vi } from "vitest";
import { api, ApiError, consumeSse, parseSseFrame } from "./client";

afterEach(() => vi.unstubAllGlobals());

describe("parseSseFrame", () => {
  it("preserves HTTP status and server error code", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response(JSON.stringify({
      code: "rate_limited",
      message: "owner API limit exceeded",
    }), { status: 429, headers: { "content-type": "application/json" } })));

    await expect(api("/api/v1/test")).rejects.toEqual(expect.objectContaining<ApiError>({
      name: "ApiError",
      status: 429,
      code: "rate_limited",
      message: "owner API limit exceeded",
    }));
  });

  it("parses ids, named events, and multiline data", () => {
    expect(parseSseFrame("id: 42\nevent: message.created\ndata: {\"line\":1}\ndata: tail")).toEqual({
      id: "42",
      event: "message.created",
      data: "{\"line\":1}\ntail",
    });
  });

  it("ignores keep-alive comments", () => {
    expect(parseSseFrame(": keep-alive")).toBeNull();
  });

  it("opens SSE without the unnecessary cache-control request header", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(new ReadableStream({
      start(controller) { controller.close(); },
    }), { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);

    await consumeSse("/api/v1/companies/company-1/events", "session-token", null, new AbortController().signal, vi.fn());

    const request = fetchMock.mock.calls[0]?.[1] as RequestInit;
    const headers = new Headers(request.headers);
    expect(headers.get("accept")).toBe("text/event-stream");
    expect(headers.get("authorization")).toBe("Bearer session-token");
    expect(headers.has("cache-control")).toBe(false);
  });
});

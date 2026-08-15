import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { mergeMessages, useConversationMessages } from "./useConversationMessages";

type TestMessage = { id: string; created_at: string; content: string };

function jsonResponse(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
}

afterEach(() => vi.unstubAllGlobals());

describe("conversation message history", () => {
  it("deduplicates and orders realtime pages", () => {
    const older = { id: "1", created_at: "2026-01-01T00:00:00Z", content: "older" };
    const newer = { id: "2", created_at: "2026-01-01T00:00:01Z", content: "newer" };
    expect(mergeMessages([newer], [older, newer])).toEqual([older, newer]);
  });

  it("loads the newest page and then requests history with before_message_id", async () => {
    const fetchMock = vi.fn()
      .mockResolvedValueOnce(jsonResponse({
        messages: [{ id: "2", created_at: "2026-01-01T00:00:01Z", content: "newer" }],
        next_cursor: "2",
        has_more: true,
      }))
      .mockResolvedValueOnce(jsonResponse({
        messages: [{ id: "1", created_at: "2026-01-01T00:00:00Z", content: "older" }],
        next_cursor: null,
        has_more: false,
      }));
    vi.stubGlobal("fetch", fetchMock);

    const { result } = renderHook(() => useConversationMessages<TestMessage>({
      conversationId: "conversation-1",
      token: "token",
      realtimeEvents: [],
      onError: (error) => { throw error; },
    }));

    await waitFor(() => expect(result.current.messages.map((message) => message.id)).toEqual(["2"]));
    await act(async () => { await result.current.loadOlder(); });

    expect(result.current.messages.map((message) => message.id)).toEqual(["1", "2"]);
    expect(String(fetchMock.mock.calls[1]?.[0])).toContain("before_message_id=2");
    expect(result.current.hasMore).toBe(false);
  });

  it("refreshes when a message event is followed by another realtime event in the same render", async () => {
    const fetchMock = vi.fn()
      .mockResolvedValueOnce(jsonResponse({ messages: [], next_cursor: null, has_more: false }))
      .mockResolvedValueOnce(jsonResponse({
        messages: [{ id: "reply", created_at: "2026-01-01T00:00:01Z", content: "agent reply" }],
        next_cursor: null,
        has_more: false,
      }));
    vi.stubGlobal("fetch", fetchMock);
    const messageEvent = {
      sequence_id: 10,
      id: "event-10",
      company_id: "company-1",
      event_type: "message.created",
      aggregate_type: "conversation",
      aggregate_id: "conversation-1",
      actor_agent_id: "agent-1",
      actor_human_user_id: null,
      payload: { conversation_id: "conversation-1", message_id: "reply" },
      created_at: "2026-01-01T00:00:01Z",
    };
    const runtimeEvent = { ...messageEvent, sequence_id: 11, id: "event-11", event_type: "codex.run.updated" };

    const { result, rerender } = renderHook(({ events }) => useConversationMessages<TestMessage>({
      conversationId: "conversation-1",
      token: "token",
      realtimeEvents: events,
      onError: (error) => { throw error; },
    }), { initialProps: { events: [] as typeof messageEvent[] } });

    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));
    rerender({ events: [messageEvent, runtimeEvent] });
    await waitFor(() => expect(result.current.messages.map((message) => message.id)).toEqual(["reply"]));
  });
});

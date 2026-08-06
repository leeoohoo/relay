import { useCallback, useEffect, useState } from "react";
import { api } from "../api/client";
import type { CompanyRealtimeEvent, MessagePage } from "../api/types";

type IdentifiedMessage = { id: string; created_at: string };

export function mergeMessages<TMessage extends IdentifiedMessage>(
  current: TMessage[],
  incoming: TMessage[],
): TMessage[] {
  const byId = new Map(current.map((message) => [message.id, message]));
  for (const message of incoming) byId.set(message.id, message);
  return [...byId.values()].sort((left, right) =>
    left.created_at.localeCompare(right.created_at) || left.id.localeCompare(right.id));
}

export function useConversationMessages<TMessage extends IdentifiedMessage>(options: {
  conversationId: string;
  token: string;
  realtimeEvent: CompanyRealtimeEvent | null;
  onError: (error: unknown) => void;
}) {
  const [messages, setMessages] = useState<TMessage[]>([]);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(false);
  const [loadingOlder, setLoadingOlder] = useState(false);

  const fetchPage = useCallback(async (beforeMessageId?: string) => {
    const query = new URLSearchParams({ limit: "100" });
    if (beforeMessageId) query.set("before_message_id", beforeMessageId);
    return api<MessagePage<TMessage>>(
      `/api/v1/conversations/${options.conversationId}/messages?${query}`,
      {},
      options.token,
    );
  }, [options.conversationId, options.token]);

  const refreshLatest = useCallback(async () => {
    if (!options.conversationId) return;
    const page = await fetchPage();
    setMessages((current) => mergeMessages(current, page.messages));
    setHasMore((current) => current || page.has_more);
  }, [fetchPage, options.conversationId]);

  const loadOlder = useCallback(async () => {
    const cursor = messages[0]?.id;
    if (!cursor || !hasMore || loadingOlder) return;
    setLoadingOlder(true);
    try {
      const page = await fetchPage(cursor);
      setMessages((current) => mergeMessages(page.messages, current));
      setHasMore(page.has_more);
    } finally {
      setLoadingOlder(false);
    }
  }, [fetchPage, hasMore, loadingOlder, messages]);

  useEffect(() => {
    if (!options.conversationId) {
      setMessages([]);
      setHasMore(false);
      return;
    }
    let active = true;
    setLoading(true);
    fetchPage()
      .then((page) => {
        if (!active) return;
        setMessages(page.messages);
        setHasMore(page.has_more);
      })
      .catch((error) => { if (active) options.onError(error); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [fetchPage, options.conversationId]);

  useEffect(() => {
    const event = options.realtimeEvent;
    if (event?.event_type !== "message.created") return;
    if (event.payload.conversation_id !== options.conversationId) return;
    refreshLatest().catch(options.onError);
  }, [options.realtimeEvent, options.conversationId, options.onError, refreshLatest]);

  return {
    messages,
    hasMore,
    loading,
    loadingOlder,
    loadOlder,
    refreshLatest,
  };
}

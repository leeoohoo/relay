import { useCallback, useEffect, useRef, useState } from "react";
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
  realtimeEvents: CompanyRealtimeEvent[];
  onError: (error: unknown) => void;
}) {
  const [messages, setMessages] = useState<TMessage[]>([]);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(false);
  const [loadingOlder, setLoadingOlder] = useState(false);
  const lastRealtimeSequenceRef = useRef(0);

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
    if (!options.realtimeEvents.length) {
      lastRealtimeSequenceRef.current = 0;
      return;
    }
    if (!options.conversationId) return;
    const freshEvents = options.realtimeEvents.filter((event) =>
      event.sequence_id > lastRealtimeSequenceRef.current);
    if (!freshEvents.length) return;
    lastRealtimeSequenceRef.current = Math.max(...freshEvents.map((event) => event.sequence_id));
    const hasNewMessage = freshEvents.some((event) =>
      event.event_type === "message.created"
      && event.payload.conversation_id === options.conversationId);
    if (!hasNewMessage) return;
    refreshLatest().catch(options.onError);
  }, [options.realtimeEvents, options.conversationId, options.onError, refreshLatest]);

  useEffect(() => {
    if (!options.conversationId) return;
    const refreshAfterReconnect = () => { void refreshLatest().catch(options.onError); };
    const refreshWhenVisible = () => {
      if (document.visibilityState === "visible") refreshAfterReconnect();
    };
    window.addEventListener("online", refreshAfterReconnect);
    window.addEventListener("focus", refreshAfterReconnect);
    document.addEventListener("visibilitychange", refreshWhenVisible);
    return () => {
      window.removeEventListener("online", refreshAfterReconnect);
      window.removeEventListener("focus", refreshAfterReconnect);
      document.removeEventListener("visibilitychange", refreshWhenVisible);
    };
  }, [options.conversationId, options.onError, refreshLatest]);

  return {
    messages,
    hasMore,
    loading,
    loadingOlder,
    loadOlder,
    refreshLatest,
  };
}

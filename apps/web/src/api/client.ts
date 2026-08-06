import type { ServerSentEvent } from "./types";

export const API_BASE_URL =
  (import.meta as ImportMeta & { env?: Record<string, string> }).env?.VITE_API_BASE_URL
  ?? window.location.origin;

function apiUrl(path: string) {
  return `${API_BASE_URL.replace(/\/$/u, "")}${path}`;
}

export async function api<T = unknown>(path: string, init: RequestInit = {}, token?: string): Promise<T> {
  const headers = new Headers(init.headers);
  if (init.body && !(init.body instanceof FormData) && !headers.has("content-type")) {
    headers.set("content-type", "application/json");
  }
  if (token) headers.set("authorization", `Bearer ${token}`);
  const response = await fetch(apiUrl(path), { ...init, headers });
  const body = await response.json().catch(() => null);
  if (!response.ok) throw new Error(body?.message ?? `请求失败 (${response.status})`);
  return body as T;
}

export function parseSseFrame(frame: string): ServerSentEvent | null {
  let id: string | null = null;
  let event = "message";
  const data: string[] = [];
  for (const rawLine of frame.split(/\r?\n/u)) {
    if (!rawLine || rawLine.startsWith(":")) continue;
    const separator = rawLine.indexOf(":");
    const field = separator < 0 ? rawLine : rawLine.slice(0, separator);
    const value = separator < 0
      ? ""
      : rawLine.slice(separator + 1).replace(/^ /u, "");
    if (field === "id") id = value;
    else if (field === "event") event = value || "message";
    else if (field === "data") data.push(value);
  }
  if (!data.length) return null;
  return { id, event, data: data.join("\n") };
}

export async function consumeSse(
  path: string,
  token: string,
  lastEventId: string | null,
  signal: AbortSignal,
  onEvent: (event: ServerSentEvent) => void,
): Promise<void> {
  const headers = new Headers({
    accept: "text/event-stream",
    authorization: `Bearer ${token}`,
  });
  if (lastEventId) headers.set("last-event-id", lastEventId);
  const response = await fetch(apiUrl(path), { headers, signal });
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new Error(body?.message ?? `实时连接失败 (${response.status})`);
  }
  if (!response.body) throw new Error("实时连接没有返回可读数据流");

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  while (true) {
    const { done, value } = await reader.read();
    buffer += decoder.decode(value, { stream: !done });
    let boundary = buffer.match(/\r?\n\r?\n/u);
    while (boundary?.index !== undefined) {
      const frame = buffer.slice(0, boundary.index);
      buffer = buffer.slice(boundary.index + boundary[0].length);
      const event = parseSseFrame(frame);
      if (event) onEvent(event);
      boundary = buffer.match(/\r?\n\r?\n/u);
    }
    if (done) return;
  }
}

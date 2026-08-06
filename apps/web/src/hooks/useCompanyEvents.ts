import { useEffect, useRef, useState } from "react";
import { consumeSse } from "../api/client";
import type { CompanyRealtimeEvent } from "../api/types";

type RealtimeStatus = "idle" | "connecting" | "connected" | "reconnecting";

export function useCompanyEvents(options: {
  companyId: string | null;
  token: string | null;
  onEvent: (event: CompanyRealtimeEvent) => void;
  onError?: (error: unknown) => void;
}) {
  const onEventRef = useRef(options.onEvent);
  const onErrorRef = useRef(options.onError);
  const [status, setStatus] = useState<RealtimeStatus>("idle");
  onEventRef.current = options.onEvent;
  onErrorRef.current = options.onError;

  useEffect(() => {
    if (!options.companyId || !options.token) {
      setStatus("idle");
      return;
    }
    const controller = new AbortController();
    let lastEventId: string | null = null;
    let retryCount = 0;

    const run = async () => {
      setStatus("connecting");
      while (!controller.signal.aborted) {
        try {
          await consumeSse(
            `/api/v1/companies/${options.companyId}/events`,
            options.token!,
            lastEventId,
            controller.signal,
            (message) => {
              if (message.id) lastEventId = message.id;
              retryCount = 0;
              setStatus("connected");
              try {
                onEventRef.current(JSON.parse(message.data) as CompanyRealtimeEvent);
              } catch (error) {
                onErrorRef.current?.(error);
              }
            },
          );
          if (!controller.signal.aborted) throw new Error("实时连接已关闭");
        } catch (error) {
          if (controller.signal.aborted) return;
          onErrorRef.current?.(error);
          setStatus("reconnecting");
          retryCount += 1;
          const delay = Math.min(15_000, 500 * 2 ** Math.min(retryCount, 5));
          await new Promise<void>((resolve) => {
            const timer = window.setTimeout(resolve, delay);
            controller.signal.addEventListener("abort", () => {
              window.clearTimeout(timer);
              resolve();
            }, { once: true });
          });
        }
      }
    };
    void run();
    return () => controller.abort();
  }, [options.companyId, options.token]);

  return status;
}

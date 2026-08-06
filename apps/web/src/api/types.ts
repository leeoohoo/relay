export type CompanyRealtimeEvent = {
  sequence_id: number;
  id: string;
  company_id: string;
  event_type: string;
  aggregate_type: string;
  aggregate_id: string | null;
  actor_agent_id: string | null;
  actor_human_user_id: string | null;
  payload: Record<string, unknown>;
  created_at: string;
};

export type MessagePage<TMessage> = {
  messages: TMessage[];
  next_cursor: string | null;
  has_more: boolean;
};

export type ServerSentEvent = {
  id: string | null;
  event: string;
  data: string;
};

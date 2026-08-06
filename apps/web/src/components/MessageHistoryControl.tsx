export function MessageHistoryControl(props: {
  visible: boolean;
  loading: boolean;
  onLoad: () => void;
}) {
  if (!props.visible) return null;
  return (
    <button
      className="load-older-messages"
      type="button"
      disabled={props.loading}
      onClick={props.onLoad}
    >
      {props.loading ? "加载中…" : "加载更早消息"}
    </button>
  );
}

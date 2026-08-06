import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { MessageHistoryControl } from "./MessageHistoryControl";

describe("MessageHistoryControl", () => {
  it("loads older messages without submitting the composer", () => {
    const onLoad = vi.fn();
    render(<MessageHistoryControl visible loading={false} onLoad={onLoad} />);
    fireEvent.click(screen.getByRole("button", { name: "加载更早消息" }));
    expect(onLoad).toHaveBeenCalledOnce();
  });
});

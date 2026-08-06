import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it } from "vitest";
import { Pagination, usePagination } from "./Pagination";

function Harness() {
  const [query, setQuery] = useState("");
  const items = Array.from({ length: 25 }, (_, index) => `item-${index + 1}`)
    .filter((item) => item.includes(query));
  const pagination = usePagination(items, 10, query);
  return <><input aria-label="筛选" value={query} onChange={(event) => setQuery(event.target.value)} />
    <div>{pagination.pageItems.join(",")}</div>
    <Pagination {...pagination} onPageChange={pagination.setPage} />
  </>;
}

describe("Pagination", () => {
  it("pages items and resets when filters change", () => {
    render(<Harness />);
    expect(screen.getByText("1–10 / 25")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "下一页" }));
    expect(screen.getByText("11–20 / 25")).toBeInTheDocument();
    fireEvent.change(screen.getByRole("textbox", { name: "筛选" }), { target: { value: "25" } });
    expect(screen.queryByRole("navigation", { name: "分页" })).not.toBeInTheDocument();
    expect(screen.getByText("item-25")).toBeInTheDocument();
  });
});

import { useEffect, useMemo, useState } from "react";
import { Icon } from "./ui";

export function usePagination<T>(items: T[], pageSize = 10, resetKey = "") {
  const [page, setPage] = useState(1);
  const pageCount = Math.max(1, Math.ceil(items.length / pageSize));

  useEffect(() => {
    setPage(1);
  }, [pageSize, resetKey]);

  useEffect(() => {
    if (page > pageCount) setPage(pageCount);
  }, [page, pageCount]);

  const pageItems = useMemo(() => {
    const start = (page - 1) * pageSize;
    return items.slice(start, start + pageSize);
  }, [items, page, pageSize]);

  return { page, pageCount, pageItems, pageSize, setPage, total: items.length };
}

export function Pagination(props: {
  page: number;
  pageCount: number;
  pageSize: number;
  total: number;
  onPageChange: (page: number) => void;
  compact?: boolean;
}) {
  if (props.total <= props.pageSize) return null;
  const start = (props.page - 1) * props.pageSize + 1;
  const end = Math.min(props.page * props.pageSize, props.total);
  const pageNumbers = paginationWindow(props.page, props.pageCount);

  return (
    <nav className={`pagination ${props.compact ? "compact" : ""}`} aria-label="分页">
      <span className="pagination-range">{start}–{end} / {props.total}</span>
      <div className="pagination-controls">
        <button type="button" aria-label="上一页" disabled={props.page <= 1} onClick={() => props.onPageChange(props.page - 1)}>
          <Icon name="chevron-left" />
        </button>
        {pageNumbers.map((item, index) => item === "ellipsis"
          ? <span className="pagination-ellipsis" key={`ellipsis-${index}`}>···</span>
          : <button
              type="button"
              className={item === props.page ? "active" : ""}
              aria-current={item === props.page ? "page" : undefined}
              onClick={() => props.onPageChange(item)}
              key={item}
            >{item}</button>)}
        <button type="button" aria-label="下一页" disabled={props.page >= props.pageCount} onClick={() => props.onPageChange(props.page + 1)}>
          <Icon name="chevron-right" />
        </button>
      </div>
    </nav>
  );
}

function paginationWindow(page: number, pageCount: number): Array<number | "ellipsis"> {
  if (pageCount <= 7) return Array.from({ length: pageCount }, (_, index) => index + 1);
  const visible = new Set([1, pageCount, page - 1, page, page + 1].filter((item) => item >= 1 && item <= pageCount));
  const ordered = [...visible].sort((left, right) => left - right);
  const result: Array<number | "ellipsis"> = [];
  ordered.forEach((item, index) => {
    const previous = ordered[index - 1];
    if (previous && item - previous > 1) result.push("ellipsis");
    result.push(item);
  });
  return result;
}

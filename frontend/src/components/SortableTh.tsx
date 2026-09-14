import type { SortDir } from "../hooks";

interface Props<K extends string> {
  label: string;
  column: K;
  sortKey: K;
  sortDir: SortDir;
  onSort: (key: K) => void;
}

export function SortableTh<K extends string>({ label, column, sortKey, sortDir, onSort }: Props<K>) {
  const active = sortKey === column;
  return (
    <th className="sortable-th" onClick={() => onSort(column)}>
      {label}
      {active && <span className="sort-arrow">{sortDir === "asc" ? "▲" : "▼"}</span>}
    </th>
  );
}

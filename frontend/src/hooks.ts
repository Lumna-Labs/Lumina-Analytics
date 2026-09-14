import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "./api";
import type { WatchlistItemType } from "./api";
import { subscribeLiveEvents, subscribeLiveStatus } from "./live";
import type { LiveEvent } from "./live";

interface AsyncState<T> {
  data: T | null;
  error: string | null;
  loading: boolean;
}

/**
 * Fetches `fn()` on mount and whenever `deps` change, and re-polls every
 * `pollMs` (default 60s, matching the ingest cycle) so dashboards stay
 * live without a manual refresh. The returned `refetch` runs `fn()`
 * immediately without waiting for the next poll tick or resetting the
 * timer — used by pages that also listen for live events (see
 * `useLiveEvents`) to refresh right away instead of waiting up to `pollMs`.
 */
export function usePolled<T>(
  fn: () => Promise<T>,
  deps: unknown[],
  pollMs = 60_000,
): AsyncState<T> & { refetch: () => void } {
  const [state, setState] = useState<AsyncState<T>>({ data: null, error: null, loading: true });
  const fnRef = useRef(fn);
  fnRef.current = fn;
  const runRef = useRef<(isFirst: boolean) => void>(() => {});

  useEffect(() => {
    let cancelled = false;

    async function run(isFirst: boolean) {
      if (isFirst) setState((s) => ({ ...s, loading: true }));
      try {
        const data = await fnRef.current();
        if (!cancelled) setState({ data, error: null, loading: false });
      } catch (e) {
        if (!cancelled) setState((s) => ({ ...s, error: (e as Error).message, loading: false }));
      }
    }

    runRef.current = (isFirst) => {
      if (!cancelled) run(isFirst);
    };
    run(true);
    const id = setInterval(() => run(false), pollMs);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);

  const refetch = useCallback(() => runRef.current(false), []);

  return { ...state, refetch };
}

/**
 * Subscribes to the shared `/events` live feed (see `live.ts`) for the
 * lifetime of the component; `onEvent` fires for every event received while
 * mounted. A thin wrapper so pages don't need to know about the module-level
 * singleton connection underneath.
 */
export function useLiveEvents(onEvent: (event: LiveEvent) => void): void {
  const handlerRef = useRef(onEvent);
  useEffect(() => {
    handlerRef.current = onEvent;
  });
  useEffect(() => subscribeLiveEvents((e) => handlerRef.current(e)), []);
}

/** Whether the shared live-event connection is currently open. */
export function useLiveStatus(): boolean {
  const [connected, setConnected] = useState(false);
  useEffect(() => subscribeLiveStatus(setConnected), []);
  return connected;
}

/**
 * Watchlist pin/unpin state for one item type (pool/token/lending_position),
 * shared by a whole page's table rather than fetched per-row: one
 * `usePolled(api.watchlist)` backs `isPinned`/`toggle` for every row on the
 * page.
 */
export function useWatchlist(itemType: WatchlistItemType) {
  const watchlist = usePolled(() => api.watchlist(), [], 60_000);
  const [pending, setPending] = useState<Set<string>>(new Set());

  const items = useMemo(
    () => (watchlist.data ?? []).filter((w) => w.item_type === itemType),
    [watchlist.data, itemType],
  );
  const pinnedKeys = useMemo(() => new Set(items.map((w) => w.item_key)), [items]);

  async function toggle(itemKey: string, label: string) {
    if (pending.has(itemKey)) return;
    setPending((p) => new Set(p).add(itemKey));
    try {
      const existing = items.find((w) => w.item_key === itemKey);
      if (existing) {
        await api.removeFromWatchlist(existing.id);
      } else {
        await api.addToWatchlist({ item_type: itemType, item_key: itemKey, label });
      }
      watchlist.refetch();
    } finally {
      setPending((p) => {
        const next = new Set(p);
        next.delete(itemKey);
        return next;
      });
    }
  }

  return {
    isPinned: (key: string) => pinnedKeys.has(key),
    isBusy: (key: string) => pending.has(key),
    toggle,
  };
}

export type SortDir = "asc" | "desc";

/**
 * Client-side sorting for a table of rows. Numeric-looking values (including
 * the string-encoded Decimal/NUMERIC fields the API returns) sort
 * numerically; everything else falls back to locale string comparison.
 * Clicking the same column again flips direction; a new column starts
 * descending (most tables here want "biggest/newest first" by default).
 */
export function useSortableRows<T extends object>(
  rows: T[],
  initialKey: keyof T,
  initialDir: SortDir = "desc",
) {
  const [sortKey, setSortKey] = useState<keyof T>(initialKey);
  const [sortDir, setSortDir] = useState<SortDir>(initialDir);

  const sorted = useMemo(() => {
    const copy = [...rows];
    copy.sort((a, b) => {
      const av = a[sortKey];
      const bv = b[sortKey];
      const an = typeof av === "number" ? av : parseFloat(String(av));
      const bn = typeof bv === "number" ? bv : parseFloat(String(bv));
      let cmp: number;
      if (!Number.isNaN(an) && !Number.isNaN(bn)) {
        cmp = an - bn;
      } else {
        cmp = String(av ?? "").localeCompare(String(bv ?? ""));
      }
      return sortDir === "asc" ? cmp : -cmp;
    });
    return copy;
  }, [rows, sortKey, sortDir]);

  function toggleSort(key: keyof T) {
    if (key === sortKey) {
      setSortDir((d) => (d === "asc" ? "desc" : "asc"));
    } else {
      setSortKey(key);
      setSortDir("desc");
    }
  }

  return { sorted, sortKey, sortDir, toggleSort };
}

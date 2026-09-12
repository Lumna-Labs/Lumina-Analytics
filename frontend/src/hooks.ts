import { useEffect, useRef, useState } from "react";

interface AsyncState<T> {
  data: T | null;
  error: string | null;
  loading: boolean;
}

/**
 * Fetches `fn()` on mount and whenever `deps` change, and re-polls every
 * `pollMs` (default 60s, matching the ingest cycle) so dashboards stay
 * live without a manual refresh.
 */
export function usePolled<T>(fn: () => Promise<T>, deps: unknown[], pollMs = 60_000): AsyncState<T> {
  const [state, setState] = useState<AsyncState<T>>({ data: null, error: null, loading: true });
  const fnRef = useRef(fn);
  fnRef.current = fn;

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

    run(true);
    const id = setInterval(() => run(false), pollMs);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);

  return state;
}

import { useState } from "react";
import { useLiveEvents } from "../hooks";
import type { LiveEvent } from "../live";

interface Toast extends LiveEvent {
  id: number;
}

const BADGE_CLASS: Record<string, string> = {
  CRITICAL: "badge badge-critical",
  WARNING: "badge badge-warning",
  INFO: "badge badge-info",
};

let nextId = 1;
const TOAST_LIFETIME_MS = 8_000;

/**
 * App-level toast feed for live alert events (see `live.ts`). Only
 * WARNING/CRITICAL surface here — INFO-severity alerts (e.g. a whale payment
 * right at the threshold) are common enough that popping a toast for every
 * one would just be noise; they're still on the Alerts page immediately via
 * `useLiveEvents`-triggered refetch there.
 */
export function LiveToaster() {
  const [toasts, setToasts] = useState<Toast[]>([]);

  useLiveEvents((event) => {
    if (event.type !== "alert") return;
    if (event.severity !== "WARNING" && event.severity !== "CRITICAL") return;
    const id = nextId++;
    setToasts((t) => [...t, { ...event, id }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), TOAST_LIFETIME_MS);
  });

  if (toasts.length === 0) return null;

  return (
    <div className="toast-stack" role="status" aria-live="polite">
      {toasts.map((t) => (
        <div key={t.id} className="toast">
          <span className={BADGE_CLASS[t.severity ?? "INFO"]}>{t.severity}</span>
          <span className="toast-message">{t.message}</span>
          <button
            type="button"
            className="toast-dismiss"
            aria-label="Dismiss"
            onClick={() => setToasts((ts) => ts.filter((x) => x.id !== t.id))}
          >
            ×
          </button>
        </div>
      ))}
    </div>
  );
}

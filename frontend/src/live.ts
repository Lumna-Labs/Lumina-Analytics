/**
 * Shared live-event connection to the API's `/events` SSE stream (see
 * `src/events.rs` on the backend). One `EventSource` for the whole app
 * (module-level singleton) rather than one per component, since every
 * mounted component just wants a tap on the same feed.
 *
 * This is a real-time nicety layered on top of `usePolled`, never a
 * replacement for it: every event here also landed in a table the regular
 * poll already reads, so a browser without `EventSource`, a proxy that
 * blocks SSE, or a connection that never comes up just means "no live push,
 * still correct on the next poll" — see `usePolled`'s `refetch` in
 * `hooks.ts`, which live-event listeners call to skip ahead of the poll
 * timer rather than depending on this stream for correctness.
 */
import { EVENTS_URL } from "./api";

export interface LiveEvent {
  type: string;
  kind?: string;
  severity?: "INFO" | "WARNING" | "CRITICAL";
  message?: string;
  time?: string;
}

type EventListener = (event: LiveEvent) => void;
type StatusListener = (connected: boolean) => void;

const eventListeners = new Set<EventListener>();
const statusListeners = new Set<StatusListener>();
let source: EventSource | null = null;
let connected = false;

function setConnected(value: boolean) {
  if (connected === value) return;
  connected = value;
  statusListeners.forEach((l) => l(connected));
}

function ensureConnected() {
  if (source || typeof EventSource === "undefined") return;
  try {
    source = new EventSource(EVENTS_URL);
  } catch {
    return;
  }
  source.addEventListener("open", () => setConnected(true));
  // EventSource retries on its own after an error; we just reflect status.
  source.addEventListener("error", () => setConnected(false));
  source.addEventListener("alert", (e) => {
    setConnected(true);
    try {
      const data = JSON.parse((e as MessageEvent).data) as LiveEvent;
      eventListeners.forEach((l) => l(data));
    } catch {
      // Malformed payload — drop it rather than let one bad message take
      // down every other listener on the shared feed.
    }
  });
}

export function subscribeLiveEvents(listener: EventListener): () => void {
  ensureConnected();
  eventListeners.add(listener);
  return () => eventListeners.delete(listener);
}

export function subscribeLiveStatus(listener: StatusListener): () => void {
  ensureConnected();
  statusListeners.add(listener);
  listener(connected);
  return () => statusListeners.delete(listener);
}

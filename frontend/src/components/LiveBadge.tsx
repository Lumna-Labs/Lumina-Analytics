import { useLiveStatus } from "../hooks";

/** Small connection-status pill for the live-event feed (see `live.ts`).
 * Purely informational — every page still works from polling alone when
 * this reads "Offline", so this is a status light, not a warning. */
export function LiveBadge() {
  const connected = useLiveStatus();
  return (
    <div className={"pill live-badge" + (connected ? " live-on" : "")}>
      <span className="pill-dot" />
      {connected ? "Live" : "Offline"}
    </div>
  );
}

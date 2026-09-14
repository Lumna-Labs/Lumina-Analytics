/** Presentational star toggle for the Watchlist page. Pages own the actual
 * watchlist state (one `usePolled(() => api.watchlist())` per page, not one
 * fetch per row) and pass down whether this specific item is pinned — see
 * Pools.tsx/Tokens.tsx/Liquidations.tsx for the calling pattern. */
export function PinButton({
  pinned,
  onToggle,
  busy,
}: {
  pinned: boolean;
  onToggle: () => void;
  busy?: boolean;
}) {
  return (
    <button
      type="button"
      className={"pin-btn" + (pinned ? " pinned" : "")}
      onClick={(e) => {
        e.stopPropagation();
        onToggle();
      }}
      disabled={busy}
      title={pinned ? "Remove from watchlist" : "Add to watchlist"}
      aria-label={pinned ? "Remove from watchlist" : "Add to watchlist"}
    >
      {pinned ? "★" : "☆"}
    </button>
  );
}

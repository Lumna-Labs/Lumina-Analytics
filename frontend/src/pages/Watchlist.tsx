import { useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { api } from "../api";
import { usePolled } from "../hooks";
import { EmptyState, ErrorState, LoadingState } from "../components/States";
import { fmtCompact, fmtTime, truncateMiddle } from "../format";

export function Watchlist() {
  const navigate = useNavigate();
  const watchlist = usePolled(() => api.watchlist(), [], 30_000);
  const pools = usePolled(() => api.pools(), []);
  const tokens = usePolled(() => api.tokens(), []);
  const liquidations = usePolled(() => api.liquidations(), []);

  const items = watchlist.data ?? [];
  const poolMap = useMemo(
    () => new Map((pools.data ?? []).map((p) => [p.pool_id, p])),
    [pools.data],
  );
  const tokenMap = useMemo(
    () => new Map((tokens.data ?? []).map((t) => [`${t.asset_code}:${t.asset_issuer}`, t])),
    [tokens.data],
  );
  const positionMap = useMemo(
    () =>
      new Map(
        (liquidations.data?.positions ?? []).map((p) => [
          `${p.protocol}:${p.pool_contract}:${p.account}`,
          p,
        ]),
      ),
    [liquidations.data],
  );

  async function remove(id: number) {
    await api.removeFromWatchlist(id);
    watchlist.refetch();
  }

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Watchlist</h1>
        <p className="page-subtitle">
          Pools, tokens, and lending positions pinned from their pages, for an at-a-glance view.
        </p>
      </div>

      {watchlist.error && <ErrorState message={watchlist.error} />}

      {watchlist.loading && !watchlist.data ? (
        <LoadingState />
      ) : items.length === 0 ? (
        <EmptyState
          title="Nothing pinned yet"
          body="Click the star next to a pool, token, or lending position on its page to pin it here."
        />
      ) : (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Type</th>
                <th>Item</th>
                <th>Snapshot</th>
                <th>Pinned</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {items.map((item) => {
                let snapshot = "—";
                let onClick: (() => void) | undefined;
                if (item.item_type === "pool") {
                  const p = poolMap.get(item.item_key);
                  snapshot = p
                    ? `${fmtCompact(p.total_shares)} shares, ${p.trustline_count ?? "—"} trustlines`
                    : "not found (may have aged out)";
                  onClick = () => navigate(`/pools/${item.item_key}`);
                } else if (item.item_type === "token") {
                  const t = tokenMap.get(item.item_key);
                  snapshot = t
                    ? `${fmtCompact(t.amount)} supply, ${t.num_accounts ?? "—"} holders`
                    : "not found (may have aged out)";
                  const [code, issuer] = item.item_key.split(":");
                  onClick = () => navigate(`/tokens/${code}/${issuer}`);
                } else {
                  const pos = positionMap.get(item.item_key);
                  snapshot = pos
                    ? `LTV ${pos.ltv ? `${parseFloat(pos.ltv).toFixed(1)}%` : "—"}, HF ${
                        pos.health_factor ? parseFloat(pos.health_factor).toFixed(2) : "—"
                      }`
                    : "not found (may have aged out)";
                  onClick = () => navigate("/liquidations");
                }
                return (
                  <tr key={item.id} className="clickable" onClick={onClick}>
                    <td>{item.item_type.replace("_", " ")}</td>
                    <td>
                      {item.label}
                      <div className="mono" style={{ fontSize: 11, color: "var(--muted)" }}>
                        {truncateMiddle(item.item_key, 10, 6)}
                      </div>
                    </td>
                    <td>{snapshot}</td>
                    <td>{fmtTime(item.created_at)}</td>
                    <td>
                      <button
                        type="button"
                        className="export-btn"
                        onClick={(e) => {
                          e.stopPropagation();
                          remove(item.id);
                        }}
                      >
                        Remove
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { api } from "../api";
import type { PoolWithLatest } from "../api";
import { usePolled, useSortableRows } from "../hooks";
import { ErrorState, LoadingState } from "../components/States";
import { SortableTh } from "../components/SortableTh";
import { downloadCsv, toCsv } from "../csv";
import { fmtCompact, fmtTime, pairLabel } from "../format";

type SortKey = keyof PoolWithLatest;

export function Pools() {
  const navigate = useNavigate();
  const pools = usePolled(() => api.pools(), []);
  const [query, setQuery] = useState("");

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return pools.data ?? [];
    return (pools.data ?? []).filter(
      (p) =>
        p.asset_a.toLowerCase().includes(q) ||
        p.asset_b.toLowerCase().includes(q) ||
        p.pool_id.toLowerCase().includes(q),
    );
  }, [pools.data, query]);

  const { sorted, sortKey, sortDir, toggleSort } = useSortableRows<PoolWithLatest>(
    filtered,
    "total_shares",
  );

  function exportCsv() {
    const csv = toCsv(sorted, [
      "pool_id",
      "asset_a",
      "asset_b",
      "fee_bp",
      "reserve_a",
      "reserve_b",
      "total_shares",
      "trustline_count",
      "time",
    ]);
    downloadCsv("lumina-pools.csv", csv);
  }

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Pool Analytics</h1>
        <p className="page-subtitle">
          Every Stellar liquidity pool observed by the indexer, with its latest reserves and
          share supply.
        </p>
      </div>

      {pools.error && <ErrorState message={pools.error} />}

      <div className="toolbar">
        <input
          type="text"
          placeholder="Search by asset code or pool id…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          style={{ minWidth: 260 }}
        />
        <span style={{ color: "var(--muted)", fontSize: 12.5 }}>
          {filtered.length} of {pools.data?.length ?? 0} pools
        </span>
        <button type="button" className="export-btn" onClick={exportCsv} disabled={sorted.length === 0}>
          Export CSV
        </button>
      </div>

      {pools.loading && !pools.data ? (
        <LoadingState />
      ) : (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <SortableTh<SortKey> label="Pair" column="asset_a" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                <SortableTh<SortKey> label="Fee" column="fee_bp" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                <SortableTh<SortKey> label="Reserve A" column="reserve_a" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                <SortableTh<SortKey> label="Reserve B" column="reserve_b" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                <SortableTh<SortKey> label="Total Shares" column="total_shares" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                <SortableTh<SortKey> label="Trustlines" column="trustline_count" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                <SortableTh<SortKey> label="Last Snapshot" column="time" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
              </tr>
            </thead>
            <tbody>
              {sorted.map((p) => (
                <tr key={p.pool_id} className="clickable" onClick={() => navigate(`/pools/${p.pool_id}`)}>
                  <td>{pairLabel(p.asset_a, p.asset_b)}</td>
                  <td>{(p.fee_bp / 100).toFixed(2)}%</td>
                  <td>{fmtCompact(p.reserve_a)}</td>
                  <td>{fmtCompact(p.reserve_b)}</td>
                  <td>{fmtCompact(p.total_shares)}</td>
                  <td>{p.trustline_count ?? "—"}</td>
                  <td>{fmtTime(p.time)}</td>
                </tr>
              ))}
              {filtered.length === 0 && (
                <tr>
                  <td colSpan={7}>No pools match "{query}".</td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

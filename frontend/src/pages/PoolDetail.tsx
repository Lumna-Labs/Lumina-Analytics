import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { api } from "../api";
import { usePolled } from "../hooks";
import { StatTile } from "../components/StatTile";
import { TimeSeriesChart } from "../components/TimeSeriesChart";
import { ErrorState, LoadingState } from "../components/States";
import { assetLabel, fmtCompact, pairLabel } from "../format";

const RANGES = [
  { label: "24h", hours: 24 },
  { label: "7d", hours: 24 * 7 },
  { label: "30d", hours: 24 * 30 },
];

export function PoolDetail() {
  const { poolId = "" } = useParams();
  const [hours, setHours] = useState(24);

  const pools = usePolled(() => api.pools(), []);
  const history = usePolled(() => api.poolHistory(poolId, hours), [poolId, hours]);

  const meta = pools.data?.find((p) => p.pool_id === poolId);

  const sharesSeries = history.data?.map((h) => ({ x: h.time, y: parseFloat(h.total_shares) })) ?? [];
  const reserveASeries = history.data?.map((h) => ({ x: h.time, y: parseFloat(h.reserve_a) })) ?? [];
  const reserveBSeries = history.data?.map((h) => ({ x: h.time, y: parseFloat(h.reserve_b) })) ?? [];

  const latest = history.data?.at(-1);
  const impliedPrice =
    latest && parseFloat(latest.reserve_a) !== 0
      ? parseFloat(latest.reserve_b) / parseFloat(latest.reserve_a)
      : null;

  return (
    <div>
      <Link to="/pools" className="back-link">
        ← All pools
      </Link>
      <div className="page-header">
        <h1 className="page-title">{meta ? pairLabel(meta.asset_a, meta.asset_b) : "Pool"}</h1>
        <p className="page-subtitle mono">{poolId}</p>
      </div>

      {(pools.error || history.error) && <ErrorState message={pools.error ?? history.error ?? ""} />}

      <div className="grid grid-stats">
        <StatTile label="Fee Tier" value={meta ? `${(meta.fee_bp / 100).toFixed(2)}%` : "…"} />
        <StatTile label="Trustlines" value={meta?.trustline_count != null ? String(meta.trustline_count) : "…"} />
        <StatTile label="Total Shares" value={latest ? fmtCompact(latest.total_shares) : "…"} />
        <StatTile
          label={meta ? `Implied Price (${assetLabel(meta.asset_b)} per ${assetLabel(meta.asset_a)})` : "Implied Price"}
          value={impliedPrice !== null ? impliedPrice.toPrecision(6) : "…"}
        />
      </div>

      <div className="toolbar">
        {RANGES.map((r) => (
          <button
            key={r.hours}
            onClick={() => setHours(r.hours)}
            className="pill"
            style={{
              border: "1px solid var(--border)",
              background: hours === r.hours ? "var(--series-1)" : "var(--surface)",
              color: hours === r.hours ? "white" : "var(--text-secondary)",
              cursor: "pointer",
            }}
          >
            {r.label}
          </button>
        ))}
      </div>

      <div className="card" style={{ marginBottom: 14 }}>
        <p className="card-title">Total Shares</p>
        {history.loading && !history.data ? (
          <LoadingState />
        ) : sharesSeries.length > 0 ? (
          <TimeSeriesChart data={sharesSeries} valueLabel="total shares" height={200} />
        ) : (
          <LoadingState label="No snapshots in this window yet." />
        )}
      </div>

      <div className="grid grid-2">
        <div className="card">
          <p className="card-title">Reserve — {meta ? assetLabel(meta.asset_a) : "Asset A"}</p>
          {reserveASeries.length > 0 ? (
            <TimeSeriesChart data={reserveASeries} valueLabel="reserve" height={180} />
          ) : (
            <LoadingState label="—" />
          )}
        </div>
        <div className="card">
          <p className="card-title">Reserve — {meta ? assetLabel(meta.asset_b) : "Asset B"}</p>
          {reserveBSeries.length > 0 ? (
            <TimeSeriesChart data={reserveBSeries} valueLabel="reserve" height={180} />
          ) : (
            <LoadingState label="—" />
          )}
        </div>
      </div>
    </div>
  );
}

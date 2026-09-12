import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { api } from "../api";
import { usePolled } from "../hooks";
import { StatTile } from "../components/StatTile";
import { TimeSeriesChart } from "../components/TimeSeriesChart";
import { ErrorState, LoadingState } from "../components/States";
import { fmtCompact, truncateMiddle } from "../format";

const RANGES = [
  { label: "24h", hours: 24 },
  { label: "7d", hours: 24 * 7 },
  { label: "30d", hours: 24 * 30 },
];

export function TokenDetail() {
  const { assetCode = "", assetIssuer = "" } = useParams();
  const [hours, setHours] = useState(24);

  const history = usePolled(() => api.tokenHistory(assetCode, assetIssuer, hours), [assetCode, assetIssuer, hours]);

  const supplySeries = history.data?.map((h) => ({ x: h.time, y: parseFloat(h.amount) })) ?? [];
  const holdersSeries = history.data?.map((h) => ({ x: h.time, y: h.num_accounts })) ?? [];
  const latest = history.data?.at(-1);

  return (
    <div>
      <Link to="/tokens" className="back-link">
        ← All tokens
      </Link>
      <div className="page-header">
        <h1 className="page-title">{assetCode}</h1>
        <p className="page-subtitle mono">{truncateMiddle(assetIssuer, 10, 10)}</p>
      </div>

      {history.error && <ErrorState message={history.error} />}

      <div className="grid grid-stats">
        <StatTile label="Supply" value={latest ? fmtCompact(latest.amount) : "…"} />
        <StatTile label="Holders" value={latest ? String(latest.num_accounts) : "…"} />
        <StatTile
          label="Claimable Balances"
          value={latest ? String(latest.num_claimable_balances) : "…"}
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

      <div className="grid grid-2">
        <div className="card">
          <p className="card-title">Supply</p>
          {history.loading && !history.data ? (
            <LoadingState />
          ) : supplySeries.length > 0 ? (
            <TimeSeriesChart data={supplySeries} valueLabel="supply" height={200} />
          ) : (
            <LoadingState label="No snapshots in this window yet." />
          )}
        </div>
        <div className="card">
          <p className="card-title">Holder Accounts</p>
          {holdersSeries.length > 0 ? (
            <TimeSeriesChart data={holdersSeries} valueLabel="holders" height={200} />
          ) : (
            <LoadingState label="—" />
          )}
        </div>
      </div>
    </div>
  );
}

import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { api } from "../api";
import type { AlertRow } from "../api";
import { usePolled } from "../hooks";
import { StatTile } from "../components/StatTile";
import { TimeSeriesChart } from "../components/TimeSeriesChart";
import { ErrorState, LoadingState } from "../components/States";
import { fmtCompact, fmtRelative, fmtTime, truncateMiddle } from "../format";

const ALERT_BADGE_CLASS: Record<AlertRow["severity"], string> = {
  CRITICAL: "badge badge-critical",
  WARNING: "badge badge-warning",
  INFO: "badge badge-info",
};

const RANGES = [
  { label: "24h", hours: 24 },
  { label: "7d", hours: 24 * 7 },
  { label: "30d", hours: 24 * 30 },
];

export function TokenDetail() {
  const { assetCode = "", assetIssuer = "" } = useParams();
  const [hours, setHours] = useState(24);

  const history = usePolled(() => api.tokenHistory(assetCode, assetIssuer, hours), [assetCode, assetIssuer, hours]);
  const alerts = usePolled(
    () => api.alerts(20, { assetCode, assetIssuer }),
    [assetCode, assetIssuer],
    20_000,
  );

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

      <div className="card" style={{ marginTop: 14 }}>
        <p className="card-title">Recent Alerts</p>
        {alerts.loading && !alerts.data ? (
          <LoadingState />
        ) : (alerts.data ?? []).length === 0 ? (
          <p style={{ color: "var(--muted)", fontSize: 13 }}>
            No holder-drop alerts on record for this token.
          </p>
        ) : (
          <ul style={{ listStyle: "none", margin: 0, padding: 0 }}>
            {(alerts.data ?? []).map((a) => (
              <li
                key={a.id}
                style={{
                  display: "flex",
                  gap: 10,
                  alignItems: "baseline",
                  padding: "6px 0",
                  borderBottom: "1px solid var(--border)",
                }}
              >
                <span className={ALERT_BADGE_CLASS[a.severity]}>{a.severity}</span>
                <span style={{ flex: 1 }}>{a.message}</span>
                <span title={fmtTime(a.time)} style={{ color: "var(--muted)", fontSize: 12 }}>
                  {fmtRelative(a.time)}
                </span>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

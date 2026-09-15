import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { api } from "../api";
import type { AlertRow } from "../api";
import { usePolled, useLiveEvents } from "../hooks";
import { EmptyState, ErrorState, LoadingState } from "../components/States";
import { downloadCsv, toCsv } from "../csv";
import { fmtRelative, fmtTime } from "../format";

const SEVERITIES = ["CRITICAL", "WARNING", "INFO"] as const;
const BADGE_CLASS: Record<AlertRow["severity"], string> = {
  CRITICAL: "badge badge-critical",
  WARNING: "badge badge-warning",
  INFO: "badge badge-info",
};

export function Alerts() {
  const alerts = usePolled(() => api.alerts(200), [], 20_000);
  const [severity, setSeverity] = useState<string>("ALL");

  useLiveEvents((event) => {
    if (event.type === "alert") alerts.refetch();
  });

  const filtered = useMemo(() => {
    const rows = alerts.data ?? [];
    return severity === "ALL" ? rows : rows.filter((a) => a.severity === severity);
  }, [alerts.data, severity]);

  function exportCsv() {
    const csv = toCsv(filtered, ["time", "severity", "kind", "message"]);
    downloadCsv("lumina-alerts.csv", csv);
  }

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Alerts</h1>
        <p className="page-subtitle">
          Outsized whale payments, lending positions crossing into a higher liquidation-risk band,
          and sudden pool liquidity drops, detected each ingest cycle. Always recorded here
          regardless of whether ALERT_WEBHOOK_URL is configured. Refreshes every 20s, plus
          instantly on a live event.
        </p>
      </div>

      {alerts.error && <ErrorState message={alerts.error} />}

      <div className="toolbar">
        <select value={severity} onChange={(e) => setSeverity(e.target.value)}>
          <option value="ALL">All severities</option>
          {SEVERITIES.map((s) => (
            <option key={s} value={s}>
              {s}
            </option>
          ))}
        </select>
        <span style={{ color: "var(--muted)", fontSize: 12.5 }}>
          {filtered.length} of {alerts.data?.length ?? 0} alerts
        </span>
        <button type="button" className="export-btn" onClick={exportCsv} disabled={filtered.length === 0}>
          Export CSV
        </button>
        <Link to="/alerts/settings" className="export-btn">
          Manage rules &amp; channels
        </Link>
      </div>

      {alerts.loading && !alerts.data ? (
        <LoadingState />
      ) : (alerts.data ?? []).length === 0 ? (
        <EmptyState
          title="No alerts yet"
          body="Alerts appear here as the ingester detects outsized whale payments, lending positions crossing into HIGH/CRITICAL liquidation risk, or a pool's liquidity dropping suddenly. Nothing to show until the next qualifying event."
        />
      ) : (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Time</th>
                <th>Severity</th>
                <th>Kind</th>
                <th>Message</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((a) => (
                <tr key={`${a.time}:${a.message}`}>
                  <td title={fmtTime(a.time)}>{fmtRelative(a.time)}</td>
                  <td>
                    <span className={BADGE_CLASS[a.severity]}>{a.severity}</span>
                  </td>
                  <td>{a.kind}</td>
                  <td>{a.message}</td>
                </tr>
              ))}
              {filtered.length === 0 && (
                <tr>
                  <td colSpan={4}>No alerts at this severity.</td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

import { Link, useParams } from "react-router-dom";
import { api } from "../api";
import { usePolled } from "../hooks";
import { ErrorState, LoadingState, EmptyState } from "../components/States";
import { downloadCsv, toCsv } from "../csv";
import { fmtCompact, fmtRelative, fmtTime, fmtUsd, truncateMiddle } from "../format";

/**
 * All recorded whale payments where this account was either side. Only
 * payments that already cleared the whale threshold at ingest time exist in
 * `whale_transactions` at all (see `src/bin/ingest.rs`), so `min_amount: 0`
 * here means "every whale payment on record for this account", not "every
 * payment this account ever made".
 */
export function AccountActivity() {
  const { address = "" } = useParams();
  const activity = usePolled(
    () => api.whaleTransactions(0, 500, address),
    [address],
    20_000,
  );

  function exportCsv() {
    const csv = toCsv(activity.data ?? [], [
      "time",
      "asset_code",
      "asset_issuer",
      "amount",
      "amount_usd",
      "source_account",
      "dest_account",
      "tx_hash",
    ]);
    downloadCsv(`lumina-account-${address}.csv`, csv);
  }

  return (
    <div>
      <Link to="/whales" className="back-link">
        ← Whale Tracker
      </Link>
      <div className="page-header">
        <h1 className="page-title">Account Activity</h1>
        <p className="page-subtitle mono">{address}</p>
      </div>

      {activity.error && <ErrorState message={activity.error} />}

      <div className="toolbar">
        <span style={{ color: "var(--muted)", fontSize: 12.5 }}>
          {activity.data ? `${activity.data.length} whale payment(s) involving this account` : ""}
        </span>
        <button
          type="button"
          className="export-btn"
          onClick={exportCsv}
          disabled={!activity.data || activity.data.length === 0}
        >
          Export CSV
        </button>
      </div>

      {activity.loading && !activity.data ? (
        <LoadingState />
      ) : activity.data && activity.data.length === 0 ? (
        <EmptyState
          title="No whale payments on record"
          body="This account hasn't sent or received a payment that cleared the whale threshold at ingest time."
        />
      ) : (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Time</th>
                <th>Direction</th>
                <th>Asset</th>
                <th>Amount</th>
                <th>USD est.</th>
                <th>Counterparty</th>
                <th>Tx</th>
              </tr>
            </thead>
            <tbody>
              {(activity.data ?? []).map((w) => {
                const sent = w.source_account === address;
                const counterparty = sent ? w.dest_account : w.source_account;
                return (
                  <tr key={w.tx_hash + w.time}>
                    <td title={fmtTime(w.time)}>{fmtRelative(w.time)}</td>
                    <td>
                      <span className={sent ? "badge badge-warning" : "badge badge-info"}>
                        {sent ? "Sent" : "Received"}
                      </span>
                    </td>
                    <td>{w.asset_code}</td>
                    <td>{fmtCompact(w.amount)}</td>
                    <td>{fmtUsd(w.amount_usd) ?? "—"}</td>
                    <td className="mono">{counterparty ? truncateMiddle(counterparty) : "—"}</td>
                    <td className="mono">{truncateMiddle(w.tx_hash, 5, 5)}</td>
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

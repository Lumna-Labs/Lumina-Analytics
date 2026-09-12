import { useState } from "react";
import { api } from "../api";
import { usePolled } from "../hooks";
import { ErrorState, LoadingState } from "../components/States";
import { fmtCompact, fmtRelative, fmtTime, fmtUsd, truncateMiddle } from "../format";

export function Whales() {
  const [minAmount, setMinAmount] = useState(10_000);
  const whales = usePolled(() => api.whaleTransactions(minAmount, 200), [minAmount], 20_000);

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Whale Tracker</h1>
        <p className="page-subtitle">
          Large payment operations network-wide, sized in each asset's own native units (no USD
          price oracle yet — a 10,000 XLM payment and a 10,000-unit micro-cap token payment both
          clear the same threshold). Refreshes every 20s.
        </p>
      </div>

      {whales.error && <ErrorState message={whales.error} />}

      <div className="toolbar">
        <label htmlFor="min-amount" style={{ color: "var(--text-secondary)", fontSize: 13 }}>
          Min amount
        </label>
        <input
          id="min-amount"
          type="number"
          value={minAmount}
          min={0}
          step={1000}
          onChange={(e) => setMinAmount(Number(e.target.value) || 0)}
          style={{ width: 130 }}
        />
        <span style={{ color: "var(--muted)", fontSize: 12.5 }}>
          {whales.data ? `${whales.data.length} matching payments` : ""}
        </span>
      </div>

      {whales.loading && !whales.data ? (
        <LoadingState />
      ) : (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Time</th>
                <th>Asset</th>
                <th>Amount</th>
                <th>USD est.</th>
                <th>From</th>
                <th>To</th>
                <th>Tx</th>
              </tr>
            </thead>
            <tbody>
              {(whales.data ?? []).map((w) => (
                <tr key={w.tx_hash + w.time}>
                  <td title={fmtTime(w.time)}>{fmtRelative(w.time)}</td>
                  <td>{w.asset_code}</td>
                  <td>{fmtCompact(w.amount)}</td>
                  <td>{fmtUsd(w.amount_usd) ?? "—"}</td>
                  <td className="mono">{truncateMiddle(w.source_account)}</td>
                  <td className="mono">{w.dest_account ? truncateMiddle(w.dest_account) : "—"}</td>
                  <td className="mono">{truncateMiddle(w.tx_hash, 5, 5)}</td>
                </tr>
              ))}
              {whales.data && whales.data.length === 0 && (
                <tr>
                  <td colSpan={7}>No payments at or above this threshold yet.</td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

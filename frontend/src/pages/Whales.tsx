import { useState } from "react";
import { api } from "../api";
import type { WhaleTransactionRow } from "../api";
import { usePolled, useSortableRows } from "../hooks";
import { ErrorState, LoadingState } from "../components/States";
import { SortableTh } from "../components/SortableTh";
import { downloadCsv, toCsv } from "../csv";
import { fmtCompact, fmtRelative, fmtTime, fmtUsd, truncateMiddle } from "../format";

type SortKey = keyof WhaleTransactionRow;

export function Whales() {
  const [minAmount, setMinAmount] = useState(10_000);
  const whales = usePolled(() => api.whaleTransactions(minAmount, 200), [minAmount], 20_000);
  const { sorted, sortKey, sortDir, toggleSort } = useSortableRows<WhaleTransactionRow>(
    whales.data ?? [],
    "time",
  );

  function exportCsv() {
    const csv = toCsv(sorted, [
      "time",
      "asset_code",
      "asset_issuer",
      "amount",
      "amount_usd",
      "source_account",
      "dest_account",
      "tx_hash",
    ]);
    downloadCsv("lumina-whale-payments.csv", csv);
  }

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
        <button type="button" className="export-btn" onClick={exportCsv} disabled={sorted.length === 0}>
          Export CSV
        </button>
      </div>

      {whales.loading && !whales.data ? (
        <LoadingState />
      ) : (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <SortableTh<SortKey> label="Time" column="time" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                <SortableTh<SortKey> label="Asset" column="asset_code" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                <SortableTh<SortKey> label="Amount" column="amount" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                <SortableTh<SortKey> label="USD est." column="amount_usd" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                <SortableTh<SortKey> label="From" column="source_account" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                <SortableTh<SortKey> label="To" column="dest_account" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                <th>Tx</th>
              </tr>
            </thead>
            <tbody>
              {sorted.map((w) => (
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

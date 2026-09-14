import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { api } from "../api";
import type { TokenWithLatest } from "../api";
import { usePolled, useSortableRows } from "../hooks";
import { ErrorState, LoadingState } from "../components/States";
import { SortableTh } from "../components/SortableTh";
import { downloadCsv, toCsv } from "../csv";
import { fmtCompact, fmtTime, truncateMiddle } from "../format";

type SortKey = keyof TokenWithLatest;

export function Tokens() {
  const navigate = useNavigate();
  const tokens = usePolled(() => api.tokens(), []);
  const [query, setQuery] = useState("");

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return tokens.data ?? [];
    return (tokens.data ?? []).filter(
      (t) => t.asset_code.toLowerCase().includes(q) || t.asset_issuer.toLowerCase().includes(q),
    );
  }, [tokens.data, query]);

  const { sorted, sortKey, sortDir, toggleSort } = useSortableRows<TokenWithLatest>(
    filtered,
    "num_accounts",
  );

  function exportCsv() {
    const csv = toCsv(sorted, [
      "asset_code",
      "asset_issuer",
      "amount",
      "num_accounts",
      "num_claimable_balances",
      "time",
    ]);
    downloadCsv("lumina-tokens.csv", csv);
  }

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Token Analysis</h1>
        <p className="page-subtitle">
          Issued assets observed on Stellar, ranked by number of holding accounts.
        </p>
      </div>

      {tokens.error && <ErrorState message={tokens.error} />}

      <div className="toolbar">
        <input
          type="text"
          placeholder="Search by asset code or issuer…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          style={{ minWidth: 260 }}
        />
        <span style={{ color: "var(--muted)", fontSize: 12.5 }}>
          {filtered.length} of {tokens.data?.length ?? 0} assets
        </span>
        <button type="button" className="export-btn" onClick={exportCsv} disabled={sorted.length === 0}>
          Export CSV
        </button>
      </div>

      {tokens.loading && !tokens.data ? (
        <LoadingState />
      ) : (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <SortableTh<SortKey> label="Code" column="asset_code" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                <SortableTh<SortKey> label="Issuer" column="asset_issuer" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                <SortableTh<SortKey> label="Supply" column="amount" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                <SortableTh<SortKey> label="Holders" column="num_accounts" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                <SortableTh<SortKey> label="Claimable Balances" column="num_claimable_balances" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                <SortableTh<SortKey> label="Last Snapshot" column="time" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
              </tr>
            </thead>
            <tbody>
              {sorted.map((t) => (
                <tr
                  key={`${t.asset_code}:${t.asset_issuer}`}
                  className="clickable"
                  onClick={() => navigate(`/tokens/${t.asset_code}/${t.asset_issuer}`)}
                >
                  <td>{t.asset_code}</td>
                  <td className="mono">{truncateMiddle(t.asset_issuer)}</td>
                  <td>{fmtCompact(t.amount)}</td>
                  <td>{t.num_accounts ?? "—"}</td>
                  <td>{t.num_claimable_balances ?? "—"}</td>
                  <td>{fmtTime(t.time)}</td>
                </tr>
              ))}
              {filtered.length === 0 && (
                <tr>
                  <td colSpan={6}>No assets match "{query}".</td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

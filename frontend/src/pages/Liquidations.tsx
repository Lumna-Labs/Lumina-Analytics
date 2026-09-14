import { api } from "../api";
import { usePolled, useWatchlist } from "../hooks";
import { EmptyState, ErrorState, LoadingState } from "../components/States";
import { PinButton } from "../components/PinButton";
import { riskColor, useChartTheme } from "../theme";
import { fmtCompact, truncateMiddle } from "../format";

export function Liquidations() {
  const theme = useChartTheme();
  const liquidations = usePolled(() => api.liquidations(), []);
  const watchlist = useWatchlist("lending_position");

  const summary = liquidations.data?.summary ?? [];
  const positions = liquidations.data?.positions ?? [];

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Liquidation Risk</h1>
        <p className="page-subtitle">
          Lending positions bucketed by loan-to-value, sourced from Blend pool events on Soroban
          when BLEND_POOL_IDS is configured.
        </p>
      </div>

      {liquidations.error && <ErrorState message={liquidations.error} />}
      {liquidations.data?.note && (
        <p className="page-subtitle" style={{ marginTop: -8 }}>
          {liquidations.data.note}
        </p>
      )}

      {liquidations.loading && !liquidations.data ? (
        <LoadingState />
      ) : positions.length === 0 ? (
        <EmptyState
          title="No lending positions ingested yet"
          body="This dashboard reads from the lending_positions table. Set BLEND_POOL_IDS on lumina-ingest to real, verified Blend pool contract addresses to start tracking on-chain borrow/supply activity — until then this page stays empty rather than showing invented numbers."
        />
      ) : (
        <>
          {summary.length > 0 && (
            <div className="grid grid-stats">
              {summary.map((s) => (
                <div className="card" key={s.risk_level}>
                  <p className="card-title" style={{ color: riskColor(s.risk_level, theme) }}>
                    {s.risk_level}
                  </p>
                  <div className="stat-value">{s.position_count}</div>
                  <div className="stat-sub">{fmtCompact(s.total_debt)} debt</div>
                </div>
              ))}
            </div>
          )}

          <h2 className="section-title">Positions</h2>
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th></th>
                  <th>Account</th>
                  <th>Pool</th>
                  <th>Collateral</th>
                  <th>Debt</th>
                  <th>LTV</th>
                  <th>Health Factor</th>
                </tr>
              </thead>
              <tbody>
                {positions.map((p) => {
                  const key = `${p.protocol}:${p.pool_contract}:${p.account}`;
                  return (
                    <tr key={key}>
                      <td>
                        <PinButton
                          pinned={watchlist.isPinned(key)}
                          busy={watchlist.isBusy(key)}
                          onToggle={() => watchlist.toggle(key, truncateMiddle(p.account))}
                        />
                      </td>
                      <td className="mono">{truncateMiddle(p.account)}</td>
                      <td className="mono">{truncateMiddle(p.pool_contract)}</td>
                      <td className="mono">
                        {fmtCompact(p.collateral_amount)} {truncateMiddle(p.collateral_asset, 4, 4)}
                      </td>
                      <td className="mono">
                        {fmtCompact(p.debt_amount)} {truncateMiddle(p.debt_asset, 4, 4)}
                      </td>
                      <td>{p.ltv ? `${parseFloat(p.ltv).toFixed(1)}%` : "—"}</td>
                      <td>{p.health_factor ? parseFloat(p.health_factor).toFixed(2) : "—"}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </>
      )}
    </div>
  );
}

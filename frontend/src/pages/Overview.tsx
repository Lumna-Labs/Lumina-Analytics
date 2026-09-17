import { Link, useNavigate } from "react-router-dom";
import { api } from "../api";
import { usePolled } from "../hooks";
import { StatTile } from "../components/StatTile";
import { TimeSeriesChart } from "../components/TimeSeriesChart";
import { ErrorState, LoadingState } from "../components/States";
import { fmtCompact, fmtPct, fmtTime, fmtUsd, pairLabel, truncateMiddle } from "../format";

export function Overview() {
  const navigate = useNavigate();
  const tvl = usePolled(() => api.tvl(24), []);
  const pools = usePolled(() => api.pools(), []);
  const tokens = usePolled(() => api.tokens(), []);
  const trending = usePolled(() => api.poolsTrending(24), []);
  const tokenTrending = usePolled(() => api.tokensTrending(24), []);
  const whales = usePolled(() => api.whaleTransactions(10_000, 10), []);

  const series = tvl.data?.map((p) => ({ x: p.bucket, y: parseFloat(p.total_reserve_native) })) ?? [];
  const latest = tvl.data?.at(-1);
  const first = tvl.data?.[0];
  const changePct =
    latest && first && parseFloat(first.total_reserve_native) !== 0
      ? ((parseFloat(latest.total_reserve_native) - parseFloat(first.total_reserve_native)) /
          parseFloat(first.total_reserve_native)) *
        100
      : null;

  const latestUsd = fmtUsd(latest?.total_reserve_usd);
  const topPools = pools.data?.slice(0, 8) ?? [];
  const gainers = (trending.data ?? [])
    .filter((t) => t.shares_change_pct !== null)
    .slice(0, 6);
  const tokenGainers = (tokenTrending.data ?? [])
    .filter((t) => t.holders_change_pct !== null)
    .slice(0, 6);

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">TVL Overview</h1>
        <p className="page-subtitle">
          Stellar classic-AMM liquidity, tracked network-wide via Horizon, refreshed every 60s.
          {latestUsd
            ? " USD figures are derived from XLM/USD spot price × pool reserve ratios, not a full price oracle."
            : " Figures are in native reserve units per asset — enable PRICE_FEED_ENABLED on the ingester for USD."}
        </p>
      </div>

      {(tvl.error || pools.error) && <ErrorState message={tvl.error ?? pools.error ?? ""} />}

      <div className="grid grid-stats">
        <StatTile
          label={latestUsd ? "XLM Locked (USD est.)" : "Native XLM Locked"}
          value={latest ? (latestUsd ?? fmtCompact(latest.total_reserve_native)) : "…"}
          sub={
            latestUsd && latest
              ? `${fmtCompact(latest.total_reserve_native)} XLM`
              : changePct !== null
                ? `${fmtPct(changePct)} over window`
                : "across pools with an XLM leg"
          }
        />
        <StatTile
          label="Pools Tracked"
          value={pools.data ? String(pools.data.length) : "…"}
          sub="Stellar liquidity pools"
        />
        <StatTile
          label="Assets Tracked"
          value={tokens.data ? String(tokens.data.length) : "…"}
          sub="issued assets with a snapshot"
        />
        <StatTile
          label="Whale Payments"
          value={whales.data ? String(whales.data.length) : "…"}
          sub="most recent large transfers"
        />
      </div>

      <div className="card">
        <p className="card-title">Native XLM Reserves Locked (24h)</p>
        {tvl.loading && !tvl.data ? (
          <LoadingState />
        ) : series.length > 0 ? (
          <TimeSeriesChart data={series} valueLabel="XLM reserves" />
        ) : (
          <LoadingState label="No snapshots in this window yet — check back after the next ingest cycle." />
        )}
      </div>

      <h2 className="section-title">Top Pools by Liquidity</h2>
      <div className="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Pair</th>
              <th>Fee</th>
              <th>Total Shares</th>
              <th>Trustlines</th>
              <th>Last Snapshot</th>
            </tr>
          </thead>
          <tbody>
            {topPools.map((p) => (
              <tr key={p.pool_id} className="clickable" onClick={() => navigate(`/pools/${p.pool_id}`)}>
                <td>{pairLabel(p.asset_a, p.asset_b)}</td>
                <td>{(p.fee_bp / 100).toFixed(2)}%</td>
                <td>{fmtCompact(p.total_shares)}</td>
                <td>{p.trustline_count ?? "—"}</td>
                <td>{fmtTime(p.time)}</td>
              </tr>
            ))}
            {pools.data && topPools.length === 0 && (
              <tr>
                <td colSpan={5}>No pools ingested yet.</td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <h2 className="section-title">Trending — Growing Total Shares (24h)</h2>
      <div className="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Pair</th>
              <th>Shares Change</th>
              <th>Reserve A</th>
              <th>Reserve B</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {gainers.map((t) => (
              <tr key={t.pool_id}>
                <td>{pairLabel(t.asset_a, t.asset_b)}</td>
                <td style={{ color: parseFloat(t.shares_change_pct ?? "0") >= 0 ? "var(--good)" : "var(--critical)" }}>
                  {fmtPct(t.shares_change_pct)}
                </td>
                <td>{fmtCompact(t.reserve_a_now)}</td>
                <td>{fmtCompact(t.reserve_b_now)}</td>
                <td>
                  <Link to={`/pools/${t.pool_id}`}>view →</Link>
                </td>
              </tr>
            ))}
            {trending.data && gainers.length === 0 && (
              <tr>
                <td colSpan={5}>
                  Not enough history yet to compute trends — this fills in as more ingest cycles run.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <h2 className="section-title">Trending — Growing Holders (24h)</h2>
      <div className="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Asset</th>
              <th>Holders Change</th>
              <th>Holders Now</th>
              <th>Supply Now</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {tokenGainers.map((t) => (
              <tr key={t.asset_code + t.asset_issuer}>
                <td>{t.asset_code}</td>
                <td
                  style={{
                    color: parseFloat(t.holders_change_pct ?? "0") >= 0 ? "var(--good)" : "var(--critical)",
                  }}
                >
                  {fmtPct(t.holders_change_pct)}
                </td>
                <td>{t.holders_now}</td>
                <td>{fmtCompact(t.amount_now)}</td>
                <td>
                  <Link to={`/tokens/${encodeURIComponent(t.asset_code)}/${encodeURIComponent(t.asset_issuer)}`}>
                    view →
                  </Link>
                </td>
              </tr>
            ))}
            {tokenTrending.data && tokenGainers.length === 0 && (
              <tr>
                <td colSpan={5}>
                  Not enough history yet to compute trends — this fills in as more ingest cycles run.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <h2 className="section-title">Recent Whale Payments</h2>
      <div className="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Time</th>
              <th>Asset</th>
              <th>Amount</th>
              <th>From</th>
              <th>To</th>
            </tr>
          </thead>
          <tbody>
            {(whales.data ?? []).map((w) => (
              <tr key={w.tx_hash + w.time}>
                <td>{fmtTime(w.time)}</td>
                <td>{w.asset_code}</td>
                <td>{fmtCompact(w.amount)}</td>
                <td className="mono">
                  <Link to={`/accounts/${w.source_account}`}>{truncateMiddle(w.source_account)}</Link>
                </td>
                <td className="mono">
                  {w.dest_account ? (
                    <Link to={`/accounts/${w.dest_account}`}>{truncateMiddle(w.dest_account)}</Link>
                  ) : (
                    "—"
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

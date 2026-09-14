const BASE_URL = import.meta.env.VITE_API_BASE_URL ?? "http://localhost:8080";

export class ApiError extends Error {}

async function get<T>(path: string): Promise<T> {
  const res = await fetch(`${BASE_URL}${path}`);
  if (!res.ok) {
    const body = await res.text();
    throw new ApiError(`${res.status} ${res.statusText}: ${body}`);
  }
  return res.json() as Promise<T>;
}

// All Decimal/NUMERIC fields arrive from the Rust API as strings to avoid
// float precision loss; components parseFloat() them at render time.

export interface TvlPoint {
  bucket: string;
  pool_count: number;
  total_reserve_native: string;
  // `null` until the price feed (PRICE_FEED_ENABLED) has recorded an XLM/USD rate.
  total_reserve_usd: string | null;
}

export interface PoolWithLatest {
  pool_id: string;
  asset_a: string;
  asset_b: string;
  fee_bp: number;
  time: string | null;
  reserve_a: string | null;
  reserve_b: string | null;
  total_shares: string | null;
  trustline_count: number | null;
}

export interface PoolSnapshotRow {
  time: string;
  reserve_a: string;
  reserve_b: string;
  total_shares: string;
  trustline_count: number;
}

export interface TokenWithLatest {
  asset_code: string;
  asset_issuer: string;
  time: string | null;
  amount: string | null;
  num_accounts: number | null;
  num_claimable_balances: number | null;
}

export interface WhaleTransactionRow {
  time: string;
  tx_hash: string;
  source_account: string;
  dest_account: string | null;
  asset_code: string;
  asset_issuer: string | null;
  amount: string;
  // `null` when this asset has never traded against native XLM in a tracked pool.
  amount_usd: string | null;
}

export interface LendingPositionRow {
  time: string;
  protocol: string;
  pool_contract: string;
  account: string;
  collateral_asset: string;
  collateral_amount: string;
  debt_asset: string;
  debt_amount: string;
  ltv: string | null;
  health_factor: string | null;
}

export interface LiquidationRiskBucket {
  risk_level: string;
  position_count: number;
  total_debt: string;
}

export interface LiquidationsResponse {
  summary: LiquidationRiskBucket[];
  positions: LendingPositionRow[];
  note: string;
}

export interface TokenSnapshotRow {
  time: string;
  amount: string;
  num_accounts: number;
  num_claimable_balances: number;
}

export interface AlertRow {
  time: string;
  kind: string;
  severity: "INFO" | "WARNING" | "CRITICAL";
  message: string;
  details: Record<string, unknown>;
}

export interface PoolTrend {
  pool_id: string;
  asset_a: string;
  asset_b: string;
  fee_bp: number;
  first_time: string;
  last_time: string;
  shares_before: string;
  shares_now: string;
  reserve_a_now: string;
  reserve_b_now: string;
  shares_change_pct: string | null;
}

export type AlertChannelKind = "slack" | "discord" | "generic";

export interface AlertChannel {
  id: number;
  name: string;
  kind: AlertChannelKind;
  url: string;
  min_severity: string;
  enabled: boolean;
  created_at: string;
}

export type AlertRuleType = "whale_threshold" | "ltv_band";

export interface AlertRule {
  id: number;
  name: string;
  rule_type: AlertRuleType;
  asset_code: string | null;
  asset_issuer: string | null;
  threshold: string;
  enabled: boolean;
  created_at: string;
}

export type WatchlistItemType = "pool" | "token" | "lending_position";

export interface WatchlistItem {
  id: number;
  item_type: WatchlistItemType;
  item_key: string;
  label: string;
  created_at: string;
}

async function send<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(`${BASE_URL}${path}`, {
    method,
    headers: body === undefined ? undefined : { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!res.ok) {
    const text = await res.text();
    throw new ApiError(`${res.status} ${res.statusText}: ${text}`);
  }
  if (res.status === 204) return undefined as T;
  return res.json() as Promise<T>;
}

/** Base URL for a raw EventSource connection — see `live.ts`. */
export const EVENTS_URL = `${BASE_URL}/events`;

export const api = {
  tvl: (hours = 24) => get<TvlPoint[]>(`/tvl?hours=${hours}`),
  pools: (limit = 2000, offset = 0) =>
    get<PoolWithLatest[]>(`/pools?limit=${limit}&offset=${offset}`),
  poolHistory: (poolId: string, hours = 24) =>
    get<PoolSnapshotRow[]>(`/pools/${poolId}/history?hours=${hours}`),
  poolsTrending: (hours = 24) => get<PoolTrend[]>(`/pools/trending?hours=${hours}`),
  tokens: (limit = 2000, offset = 0) =>
    get<TokenWithLatest[]>(`/tokens?limit=${limit}&offset=${offset}`),
  tokenHistory: (assetCode: string, assetIssuer: string, hours = 24) =>
    get<TokenSnapshotRow[]>(
      `/tokens/${encodeURIComponent(assetCode)}/${encodeURIComponent(assetIssuer)}/history?hours=${hours}`,
    ),
  whaleTransactions: (minAmount = 10_000, limit = 100, account?: string) =>
    get<WhaleTransactionRow[]>(
      `/transactions/whales?min_amount=${minAmount}&limit=${limit}` +
        (account ? `&account=${encodeURIComponent(account)}` : ""),
    ),
  liquidations: () => get<LiquidationsResponse>("/liquidations"),
  alerts: (limit = 100) => get<AlertRow[]>(`/alerts?limit=${limit}`),

  alertChannels: () => get<AlertChannel[]>("/alert-channels"),
  createAlertChannel: (body: {
    name: string;
    kind: AlertChannelKind;
    url: string;
    min_severity: string;
    enabled: boolean;
  }) => send<AlertChannel>("POST", "/alert-channels", body),
  deleteAlertChannel: (id: number) => send<void>("DELETE", `/alert-channels/${id}`),

  alertRules: () => get<AlertRule[]>("/alert-rules"),
  createAlertRule: (body: {
    name: string;
    rule_type: AlertRuleType;
    asset_code?: string | null;
    asset_issuer?: string | null;
    threshold: string;
    enabled: boolean;
  }) => send<AlertRule>("POST", "/alert-rules", body),
  updateAlertRule: (id: number, body: { threshold: string; enabled: boolean }) =>
    send<AlertRule>("PATCH", `/alert-rules/${id}`, body),
  deleteAlertRule: (id: number) => send<void>("DELETE", `/alert-rules/${id}`),

  watchlist: () => get<WatchlistItem[]>("/watchlist"),
  addToWatchlist: (body: { item_type: WatchlistItemType; item_key: string; label: string }) =>
    send<WatchlistItem>("POST", "/watchlist", body),
  removeFromWatchlist: (id: number) => send<void>("DELETE", `/watchlist/${id}`),
};

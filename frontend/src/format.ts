const compactFmt = new Intl.NumberFormat("en-US", { notation: "compact", maximumFractionDigits: 2 });
const preciseFmt = new Intl.NumberFormat("en-US", { maximumFractionDigits: 2 });
const usdCompactFmt = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
  notation: "compact",
  maximumFractionDigits: 2,
});

export function fmtCompact(value: string | number | null | undefined): string {
  if (value === null || value === undefined) return "—";
  const n = typeof value === "string" ? parseFloat(value) : value;
  if (Number.isNaN(n)) return "—";
  return compactFmt.format(n);
}

export function fmtPrecise(value: string | number | null | undefined): string {
  if (value === null || value === undefined) return "—";
  const n = typeof value === "string" ? parseFloat(value) : value;
  if (Number.isNaN(n)) return "—";
  return preciseFmt.format(n);
}

export function fmtUsd(value: string | number | null | undefined): string | null {
  if (value === null || value === undefined) return null;
  const n = typeof value === "string" ? parseFloat(value) : value;
  if (Number.isNaN(n)) return null;
  return usdCompactFmt.format(n);
}

export function fmtPct(value: string | number | null | undefined): string {
  if (value === null || value === undefined) return "—";
  const n = typeof value === "string" ? parseFloat(value) : value;
  if (Number.isNaN(n)) return "—";
  const sign = n > 0 ? "+" : "";
  return `${sign}${n.toFixed(2)}%`;
}

/**
 * Horizon encodes pool/asset identifiers as "native" or "CODE:ISSUER".
 * Renders just the tradable code, e.g. "XLM" or "USDC".
 */
export function assetLabel(asset: string): string {
  if (asset === "native") return "XLM";
  return asset.split(":")[0] ?? asset;
}

export function pairLabel(assetA: string, assetB: string): string {
  return `${assetLabel(assetA)} / ${assetLabel(assetB)}`;
}

export function truncateMiddle(s: string, head = 6, tail = 6): string {
  if (s.length <= head + tail + 1) return s;
  return `${s.slice(0, head)}…${s.slice(-tail)}`;
}

export function fmtTime(iso: string | null | undefined): string {
  if (!iso) return "—";
  const d = new Date(iso);
  return d.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function fmtRelative(iso: string | null | undefined): string {
  if (!iso) return "—";
  const ms = Date.now() - new Date(iso).getTime();
  const secs = Math.round(ms / 1000);
  if (secs < 60) return `${secs}s ago`;
  const mins = Math.round(secs / 60);
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.round(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.round(hours / 24);
  return `${days}d ago`;
}

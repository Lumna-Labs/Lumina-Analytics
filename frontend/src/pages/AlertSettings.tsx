import { useState } from "react";
import { Link } from "react-router-dom";
import { api } from "../api";
import type { AlertChannelKind, AlertRuleType } from "../api";
import { usePolled } from "../hooks";
import { ErrorState, LoadingState } from "../components/States";
import { fmtTime, truncateMiddle } from "../format";

const SEVERITIES = ["INFO", "WARNING", "CRITICAL"];

export function AlertSettings() {
  const channels = usePolled(() => api.alertChannels(), []);
  const rules = usePolled(() => api.alertRules(), []);

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Alert Rules &amp; Channels</h1>
        <p className="page-subtitle">
          Configure per-asset whale-payment thresholds, custom liquidation-risk bands, and extra
          delivery channels (Slack/Discord/generic webhooks) without a redeploy. <Link to="/alerts">Back to Alerts</Link>.
        </p>
      </div>

      <h2 className="section-title">Delivery channels</h2>
      {channels.error && <ErrorState message={channels.error} />}
      {channels.loading && !channels.data ? (
        <LoadingState />
      ) : (
        <ChannelsPanel channels={channels.data ?? []} onChanged={channels.refetch} />
      )}

      <h2 className="section-title">Rules</h2>
      {rules.error && <ErrorState message={rules.error} />}
      {rules.loading && !rules.data ? (
        <LoadingState />
      ) : (
        <RulesPanel rules={rules.data ?? []} onChanged={rules.refetch} />
      )}
    </div>
  );
}

function ChannelsPanel({
  channels,
  onChanged,
}: {
  channels: Awaited<ReturnType<typeof api.alertChannels>>;
  onChanged: () => void;
}) {
  const [name, setName] = useState("");
  const [kind, setKind] = useState<AlertChannelKind>("slack");
  const [url, setUrl] = useState("");
  const [minSeverity, setMinSeverity] = useState("WARNING");
  const [submitting, setSubmitting] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setSubmitting(true);
    setFormError(null);
    try {
      await api.createAlertChannel({ name, kind, url, min_severity: minSeverity, enabled: true });
      setName("");
      setUrl("");
      onChanged();
    } catch (err) {
      setFormError((err as Error).message);
    } finally {
      setSubmitting(false);
    }
  }

  async function remove(id: number) {
    await api.deleteAlertChannel(id);
    onChanged();
  }

  return (
    <div>
      <div className="table-wrap" style={{ marginBottom: 14 }}>
        <table>
          <thead>
            <tr>
              <th>Name</th>
              <th>Kind</th>
              <th>URL</th>
              <th>Min severity</th>
              <th>Created</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {channels.map((c) => (
              <tr key={c.id}>
                <td>{c.name}</td>
                <td>{c.kind}</td>
                <td className="mono">{truncateMiddle(c.url, 20, 6)}</td>
                <td>{c.min_severity}</td>
                <td>{fmtTime(c.created_at)}</td>
                <td>
                  <button type="button" className="export-btn" onClick={() => remove(c.id)}>
                    Remove
                  </button>
                </td>
              </tr>
            ))}
            {channels.length === 0 && (
              <tr>
                <td colSpan={6}>
                  No extra channels configured. ALERT_WEBHOOK_URL (if set) still works independently.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <form className="toolbar" onSubmit={submit}>
        <input
          type="text"
          placeholder="Channel name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          required
          style={{ minWidth: 160 }}
        />
        <select value={kind} onChange={(e) => setKind(e.target.value as AlertChannelKind)}>
          <option value="slack">Slack</option>
          <option value="discord">Discord</option>
          <option value="generic">Generic webhook</option>
        </select>
        <input
          type="text"
          placeholder="Webhook URL"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          required
          style={{ minWidth: 280 }}
        />
        <select value={minSeverity} onChange={(e) => setMinSeverity(e.target.value)}>
          {SEVERITIES.map((s) => (
            <option key={s} value={s}>
              {s}+
            </option>
          ))}
        </select>
        <button type="submit" className="export-btn" disabled={submitting}>
          Add channel
        </button>
        {formError && <span style={{ color: "var(--critical)", fontSize: 12.5 }}>{formError}</span>}
      </form>
    </div>
  );
}

function RulesPanel({
  rules,
  onChanged,
}: {
  rules: Awaited<ReturnType<typeof api.alertRules>>;
  onChanged: () => void;
}) {
  const [ruleType, setRuleType] = useState<AlertRuleType>("whale_threshold");
  const [name, setName] = useState("");
  const [assetCode, setAssetCode] = useState("");
  const [assetIssuer, setAssetIssuer] = useState("");
  const [threshold, setThreshold] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setSubmitting(true);
    setFormError(null);
    try {
      await api.createAlertRule({
        name: ruleType === "ltv_band" ? name.toUpperCase() : name || `${assetCode} threshold`,
        rule_type: ruleType,
        asset_code: ruleType === "whale_threshold" ? assetCode : null,
        asset_issuer: ruleType === "whale_threshold" && assetIssuer ? assetIssuer : null,
        threshold,
        enabled: true,
      });
      setName("");
      setAssetCode("");
      setAssetIssuer("");
      setThreshold("");
      onChanged();
    } catch (err) {
      setFormError((err as Error).message);
    } finally {
      setSubmitting(false);
    }
  }

  async function toggle(id: number, enabled: boolean, currentThreshold: string) {
    await api.updateAlertRule(id, { threshold: currentThreshold, enabled });
    onChanged();
  }

  async function remove(id: number) {
    await api.deleteAlertRule(id);
    onChanged();
  }

  return (
    <div>
      <div className="table-wrap" style={{ marginBottom: 14 }}>
        <table>
          <thead>
            <tr>
              <th>Name</th>
              <th>Type</th>
              <th>Target</th>
              <th>Threshold</th>
              <th>Enabled</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {rules.map((r) => (
              <tr key={r.id}>
                <td>{r.name}</td>
                <td>{r.rule_type === "whale_threshold" ? "Whale threshold" : "LTV band"}</td>
                <td className="mono">
                  {r.rule_type === "whale_threshold"
                    ? `${r.asset_code ?? "—"}${r.asset_issuer ? `:${truncateMiddle(r.asset_issuer, 4, 4)}` : ""}`
                    : r.name}
                </td>
                <td>{r.threshold}</td>
                <td>
                  <input
                    type="checkbox"
                    checked={r.enabled}
                    onChange={(e) => toggle(r.id, e.target.checked, r.threshold)}
                  />
                </td>
                <td>
                  <button type="button" className="export-btn" onClick={() => remove(r.id)}>
                    Remove
                  </button>
                </td>
              </tr>
            ))}
            {rules.length === 0 && (
              <tr>
                <td colSpan={6}>
                  No overrides configured. Whale payments use WHALE_THRESHOLD and liquidation risk
                  uses the default 70/85/95 LTV bands until a rule is added here.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <form className="toolbar" onSubmit={submit}>
        <select value={ruleType} onChange={(e) => setRuleType(e.target.value as AlertRuleType)}>
          <option value="whale_threshold">Whale threshold override</option>
          <option value="ltv_band">LTV band override</option>
        </select>
        {ruleType === "whale_threshold" ? (
          <>
            <input
              type="text"
              placeholder="Asset code (e.g. USDC or XLM)"
              value={assetCode}
              onChange={(e) => setAssetCode(e.target.value)}
              required
              style={{ minWidth: 140 }}
            />
            <input
              type="text"
              placeholder="Issuer (blank for native XLM)"
              value={assetIssuer}
              onChange={(e) => setAssetIssuer(e.target.value)}
              style={{ minWidth: 220 }}
            />
          </>
        ) : (
          <select value={name} onChange={(e) => setName(e.target.value)} required>
            <option value="">Band…</option>
            <option value="MEDIUM">MEDIUM</option>
            <option value="HIGH">HIGH</option>
            <option value="CRITICAL">CRITICAL</option>
          </select>
        )}
        <input
          type="number"
          placeholder={ruleType === "whale_threshold" ? "Threshold amount" : "LTV %"}
          value={threshold}
          onChange={(e) => setThreshold(e.target.value)}
          required
          step="any"
          style={{ width: 140 }}
        />
        <button type="submit" className="export-btn" disabled={submitting}>
          Add rule
        </button>
        {formError && <span style={{ color: "var(--critical)", fontSize: 12.5 }}>{formError}</span>}
      </form>
    </div>
  );
}

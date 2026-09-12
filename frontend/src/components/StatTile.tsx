interface Props {
  label: string;
  value: string;
  sub?: string;
}

export function StatTile({ label, value, sub }: Props) {
  return (
    <div className="card">
      <p className="card-title">{label}</p>
      <div className="stat-value">{value}</div>
      {sub && <div className="stat-sub">{sub}</div>}
    </div>
  );
}

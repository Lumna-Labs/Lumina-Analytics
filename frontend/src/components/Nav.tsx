import { NavLink } from "react-router-dom";

const links = [
  { to: "/", label: "TVL Overview", end: true },
  { to: "/pools", label: "Pool Analytics" },
  { to: "/tokens", label: "Token Analysis" },
  { to: "/whales", label: "Whale Tracker" },
  { to: "/liquidations", label: "Liquidations" },
];

export function Nav() {
  return (
    <aside className="sidebar">
      <div className="brand">
        <span className="brand-mark" />
        Lumina
      </div>
      <nav>
        {links.map((l) => (
          <NavLink
            key={l.to}
            to={l.to}
            end={l.end}
            className={({ isActive }) => "nav-link" + (isActive ? " active" : "")}
          >
            {l.label}
          </NavLink>
        ))}
      </nav>
    </aside>
  );
}

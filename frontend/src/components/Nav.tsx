import { NavLink } from "react-router-dom";
import { useThemeMode } from "../theme";
import type { ThemeMode } from "../theme";
import { LiveBadge } from "./LiveBadge";
import { SearchBar } from "./SearchBar";

const links = [
  { to: "/", label: "TVL Overview", end: true },
  { to: "/pools", label: "Pool Analytics" },
  { to: "/tokens", label: "Token Analysis" },
  { to: "/whales", label: "Whale Tracker" },
  { to: "/liquidations", label: "Liquidations" },
  { to: "/alerts", label: "Alerts" },
  { to: "/watchlist", label: "Watchlist" },
];

const MODES: { mode: ThemeMode; label: string }[] = [
  { mode: "system", label: "Auto" },
  { mode: "light", label: "Light" },
  { mode: "dark", label: "Dark" },
];

export function Nav() {
  const { mode, setMode } = useThemeMode();

  return (
    <aside className="sidebar">
      <div className="brand">
        <span className="brand-mark" />
        Lumina
      </div>
      <LiveBadge />
      <SearchBar />
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
      <div className="theme-toggle" role="group" aria-label="Theme">
        {MODES.map((m) => (
          <button
            key={m.mode}
            type="button"
            className={mode === m.mode ? "active" : ""}
            onClick={() => setMode(m.mode)}
          >
            {m.label}
          </button>
        ))}
      </div>
    </aside>
  );
}

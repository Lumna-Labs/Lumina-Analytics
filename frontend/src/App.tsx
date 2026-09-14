import { Route, Routes, useLocation } from "react-router-dom";
import { Nav } from "./components/Nav";
import { LiveToaster } from "./components/LiveToaster";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { Overview } from "./pages/Overview";
import { Pools } from "./pages/Pools";
import { PoolDetail } from "./pages/PoolDetail";
import { Tokens } from "./pages/Tokens";
import { TokenDetail } from "./pages/TokenDetail";
import { Whales } from "./pages/Whales";
import { AccountActivity } from "./pages/AccountActivity";
import { Liquidations } from "./pages/Liquidations";
import { Alerts } from "./pages/Alerts";
import { AlertSettings } from "./pages/AlertSettings";
import { Watchlist } from "./pages/Watchlist";

export default function App() {
  const location = useLocation();
  return (
    <div className="app-shell">
      <Nav />
      <main className="main">
        {/* Keyed by path so navigating away from a crashed page clears the
            error boundary instead of leaving the fallback stuck forever. */}
        <ErrorBoundary key={location.pathname}>
          <Routes>
            <Route path="/" element={<Overview />} />
            <Route path="/pools" element={<Pools />} />
            <Route path="/pools/:poolId" element={<PoolDetail />} />
            <Route path="/tokens" element={<Tokens />} />
            <Route path="/tokens/:assetCode/:assetIssuer" element={<TokenDetail />} />
            <Route path="/whales" element={<Whales />} />
            <Route path="/accounts/:address" element={<AccountActivity />} />
            <Route path="/liquidations" element={<Liquidations />} />
            <Route path="/alerts" element={<Alerts />} />
            <Route path="/alerts/settings" element={<AlertSettings />} />
            <Route path="/watchlist" element={<Watchlist />} />
          </Routes>
        </ErrorBoundary>
      </main>
      <LiveToaster />
    </div>
  );
}

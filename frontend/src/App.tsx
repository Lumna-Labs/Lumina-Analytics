import { Route, Routes } from "react-router-dom";
import { Nav } from "./components/Nav";
import { Overview } from "./pages/Overview";
import { Pools } from "./pages/Pools";
import { PoolDetail } from "./pages/PoolDetail";
import { Tokens } from "./pages/Tokens";
import { TokenDetail } from "./pages/TokenDetail";
import { Whales } from "./pages/Whales";
import { Liquidations } from "./pages/Liquidations";

export default function App() {
  return (
    <div className="app-shell">
      <Nav />
      <main className="main">
        <Routes>
          <Route path="/" element={<Overview />} />
          <Route path="/pools" element={<Pools />} />
          <Route path="/pools/:poolId" element={<PoolDetail />} />
          <Route path="/tokens" element={<Tokens />} />
          <Route path="/tokens/:assetCode/:assetIssuer" element={<TokenDetail />} />
          <Route path="/whales" element={<Whales />} />
          <Route path="/liquidations" element={<Liquidations />} />
        </Routes>
      </main>
    </div>
  );
}

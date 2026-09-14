import { createContext, createElement, useContext, useEffect, useState } from "react";
import type { ReactNode } from "react";

export interface ChartTheme {
  surface: string;
  page: string;
  textPrimary: string;
  textSecondary: string;
  muted: string;
  gridline: string;
  baseline: string;
  series: readonly string[];
  status: { good: string; warning: string; serious: string; critical: string };
}

// Validated categorical/status palette (see dataviz skill references/palette.md).
export const palette: { light: ChartTheme; dark: ChartTheme } = {
  light: {
    surface: "#fcfcfb",
    page: "#f9f9f7",
    textPrimary: "#0b0b0b",
    textSecondary: "#52514e",
    muted: "#898781",
    gridline: "#e1e0d9",
    baseline: "#c3c2b7",
    series: ["#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4", "#008300", "#4a3aa7", "#e34948"],
    status: { good: "#0ca30c", warning: "#fab219", serious: "#ec835a", critical: "#d03b3b" },
  },
  dark: {
    surface: "#1a1a19",
    page: "#0d0d0d",
    textPrimary: "#ffffff",
    textSecondary: "#c3c2b7",
    muted: "#898781",
    gridline: "#2c2c2a",
    baseline: "#383835",
    series: ["#3987e5", "#d95926", "#199e70", "#c98500", "#d55181", "#008300", "#9085e9", "#e66767"],
    status: { good: "#0ca30c", warning: "#fab219", serious: "#ec835a", critical: "#d03b3b" },
  },
} as const;

export type ThemeMode = "system" | "light" | "dark";
const STORAGE_KEY = "lumina-theme";

function readStoredMode(): ThemeMode {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    if (v === "light" || v === "dark" || v === "system") return v;
  } catch {
    // localStorage unavailable (private browsing, blocked storage) — fall back to system.
  }
  return "system";
}

function useSystemDark(): boolean {
  const [dark, setDark] = useState(
    () => window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? false,
  );
  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const listener = (e: MediaQueryListEvent) => setDark(e.matches);
    mq.addEventListener("change", listener);
    return () => mq.removeEventListener("change", listener);
  }, []);
  return dark;
}

interface ThemeContextValue {
  mode: ThemeMode;
  setMode: (mode: ThemeMode) => void;
  isDark: boolean;
}

const ThemeContext = createContext<ThemeContextValue | null>(null);

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [mode, setModeState] = useState<ThemeMode>(readStoredMode);
  const systemDark = useSystemDark();
  const isDark = mode === "dark" || (mode === "system" && systemDark);

  useEffect(() => {
    if (mode === "system") {
      document.documentElement.removeAttribute("data-theme");
    } else {
      document.documentElement.setAttribute("data-theme", mode);
    }
  }, [mode]);

  function setMode(next: ThemeMode) {
    setModeState(next);
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // ignore — theme just won't persist across reloads in this browser.
    }
  }

  return createElement(ThemeContext.Provider, { value: { mode, setMode, isDark } }, children);
}

export function useThemeMode(): ThemeContextValue {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error("useThemeMode must be used within a ThemeProvider");
  return ctx;
}

export function useChartTheme(): ChartTheme {
  const { isDark } = useThemeMode();
  return isDark ? palette.dark : palette.light;
}

export function riskColor(level: string, theme: ChartTheme): string {
  switch (level) {
    case "CRITICAL":
      return theme.status.critical;
    case "HIGH":
      return theme.status.serious;
    case "MEDIUM":
      return theme.status.warning;
    default:
      return theme.status.good;
  }
}

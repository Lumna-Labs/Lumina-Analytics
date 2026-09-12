import { useEffect, useState } from "react";

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

export function useIsDark(): boolean {
  const [isDark, setIsDark] = useState(
    () => window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? false,
  );
  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const listener = (e: MediaQueryListEvent) => setIsDark(e.matches);
    mq.addEventListener("change", listener);
    return () => mq.removeEventListener("change", listener);
  }, []);
  return isDark;
}

export function useChartTheme() {
  const isDark = useIsDark();
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

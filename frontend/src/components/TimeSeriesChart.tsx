import { Area, AreaChart, CartesianGrid, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import { useChartTheme } from "../theme";
import { fmtCompact, fmtTime } from "../format";

interface Point {
  x: string;
  y: number;
}

interface Props {
  data: Point[];
  height?: number;
  valueLabel: string;
}

export function TimeSeriesChart({ data, height = 220, valueLabel }: Props) {
  const theme = useChartTheme();

  return (
    <ResponsiveContainer width="100%" height={height}>
      <AreaChart data={data} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
        <defs>
          <linearGradient id="tsFill" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={theme.series[0]} stopOpacity={0.28} />
            <stop offset="100%" stopColor={theme.series[0]} stopOpacity={0} />
          </linearGradient>
        </defs>
        <CartesianGrid stroke={theme.gridline} vertical={false} />
        <XAxis
          dataKey="x"
          tickFormatter={(v) => fmtTime(v)}
          stroke={theme.baseline}
          tick={{ fill: theme.muted, fontSize: 11 }}
          tickLine={false}
          minTickGap={40}
        />
        <YAxis
          tickFormatter={(v) => fmtCompact(v)}
          stroke={theme.baseline}
          tick={{ fill: theme.muted, fontSize: 11 }}
          tickLine={false}
          axisLine={false}
          width={48}
        />
        <Tooltip
          formatter={(v) => [fmtCompact(v as number), valueLabel]}
          labelFormatter={(v) => fmtTime(v as string)}
          contentStyle={{
            background: theme.surface,
            border: `1px solid ${theme.gridline}`,
            borderRadius: 8,
            fontSize: 12,
          }}
          labelStyle={{ color: theme.textSecondary }}
        />
        <Area
          type="monotone"
          dataKey="y"
          stroke={theme.series[0]}
          strokeWidth={2}
          fill="url(#tsFill)"
          dot={false}
          activeDot={{ r: 4 }}
        />
      </AreaChart>
    </ResponsiveContainer>
  );
}

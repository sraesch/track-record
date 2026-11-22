import type { EChartProps } from '../charts/EChart'

// Stands in for the ECharts chart in tests (setup.ts): jsdom has no canvas.
// Lists the chart's lines with their number of points, so tests can see
// what would be drawn. How lines look is tested on the option builders.
export function EChart({ option }: EChartProps) {
  const series = [option.series ?? []].flat()
  return (
    <ul data-testid="chart">
      {series.map((line, i) => (
        <li key={i}>
          {`${String(line.name)}: ${Array.isArray(line.data) ? line.data.length : 0} points`}
        </li>
      ))}
    </ul>
  )
}

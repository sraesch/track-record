import { useMemo, useState, type PointerEvent, type ReactElement } from 'react';
import { useElementWidth } from '../utils/hooks';
import { TooltipContent } from './Tooltip';

/** Categorical slots, assigned in order. A ninth series folds into `other`. */
export type ChartSlot = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 'other';

/** CSS colour for a chart slot, resolving per theme. */
export function chartColor(slot: ChartSlot): string {
  return slot === 'other' ? 'var(--chart-other)' : `var(--chart-${slot})`;
}

const SLOTS: readonly Exclude<ChartSlot, 'other'>[] = [1, 2, 3, 4, 5, 6, 7, 8];

/** Slot for the n-th series (0-based); index 8 and beyond map to `other`. */
export function slotForIndex(index: number): ChartSlot {
  return SLOTS[index] ?? 'other';
}

export interface LegendItem {
  label: string;
  color: string;
}

export interface ChartLegendProps {
  items: readonly LegendItem[];
}

/** Legend above a plot; required for two or more series. */
export function ChartLegend({ items }: ChartLegendProps): ReactElement {
  return (
    <div className="g-legend">
      {items.map((i) => (
        <span key={i.label}><i className="g-swatch" style={{ background: i.color }} />{i.label}</span>
      ))}
    </div>
  );
}

export interface LineSeries {
  /** Stable id: keeps the series' colour when filters change the set. */
  id: string;
  label: string;
  /** One value per x label. `null` leaves a gap. */
  values: readonly (number | null)[];
  /** Fixed slot. Assign slots by entity, not by rank, so colours survive filtering. */
  slot: ChartSlot;
}

export interface LineChartProps {
  series: readonly LineSeries[];
  xLabels: readonly string[];
  /** Accessible summary of what the chart shows. */
  'aria-label': string;
  height?: number;
  /** Formats axis ticks and tooltip values. */
  formatValue?: (value: number) => string;
  /** Tooltip title for an x index. Defaults to the x label. */
  formatX?: (label: string, index: number) => string;
  /** Approximate number of horizontal gridlines; steps snap to 1, 2, 2.5 or 5 × 10ⁿ. Default 4. */
  yTicks?: number;
  /** Show the legend above the plot. Default: when there are 2+ series. */
  legend?: boolean;
  /** Start the y-axis at zero. Default true. */
  zeroBaseline?: boolean;
}

/** A "nice" tick step (1, 2, 2.5, 5 × 10ⁿ) giving about `count` ticks up to `span`. */
function niceStep(span: number, count: number): number {
  const raw = span / Math.max(1, count);
  if (raw <= 0) return 1;
  const mag = 10 ** Math.floor(Math.log10(raw));
  const norm = raw / mag;
  return (norm <= 1 ? 1 : norm <= 2 ? 2 : norm <= 2.5 ? 2.5 : norm <= 5 ? 5 : 10) * mag;
}

const PAD = { top: 8, right: 16, bottom: 22, left: 44 } as const;

/** Lines over time with a crosshair tooltip. One y-axis only. */
export function LineChart({
  series,
  xLabels,
  height = 200,
  formatValue = (v) => v.toLocaleString(),
  formatX = (l) => l,
  yTicks = 4,
  legend,
  zeroBaseline = true,
  ...aria
}: LineChartProps): ReactElement {
  const [ref, width] = useElementWidth<HTMLDivElement>(600);
  const [hover, setHover] = useState<number | null>(null);

  const all = series.flatMap((s) => s.values.filter((v): v is number => v !== null));
  const dataMax = Math.max(0, ...all);
  const dataMin = zeroBaseline ? 0 : Math.min(...all);
  const step = niceStep(dataMax - dataMin, yTicks);
  const minV = zeroBaseline ? 0 : Math.floor(dataMin / step) * step;
  const maxV = Math.max(minV + step, Math.ceil(dataMax / step) * step);
  const plotW = Math.max(1, width - PAD.left - PAD.right);
  const plotH = height - PAD.top - PAD.bottom;
  const n = xLabels.length;
  const x = (i: number): number => PAD.left + (n <= 1 ? plotW / 2 : (i / (n - 1)) * plotW);
  const y = (v: number): number => PAD.top + (1 - (v - minV) / (maxV - minV || 1)) * plotH;

  const paths = useMemo(
    () =>
      series.map((s) => {
        let d = '';
        let pen = false;
        s.values.forEach((v, i) => {
          if (v === null) { pen = false; return; }
          d += `${pen ? 'L' : 'M'}${x(i).toFixed(1)} ${y(v).toFixed(1)} `;
          pen = true;
        });
        return { s, d };
      }),
    [series, width, height, maxV, minV, n],
  );

  const ticks = Array.from({ length: Math.round((maxV - minV) / step) }, (_, i) => minV + (i + 1) * step);
  const labelEvery = Math.max(1, Math.ceil(n / Math.max(1, Math.floor(plotW / 64))));

  const onMove = (e: PointerEvent<SVGSVGElement>): void => {
    const rect = e.currentTarget.getBoundingClientRect();
    const px = e.clientX - rect.left - PAD.left;
    const i = n <= 1 ? 0 : Math.round((px / plotW) * (n - 1));
    setHover(Math.min(n - 1, Math.max(0, i)));
  };

  const showLegend = legend ?? series.length > 1;
  const hx = hover !== null ? x(hover) : 0;
  const hoverLabel = hover !== null ? xLabels[hover] : undefined;

  return (
    <div className="g-chart-wrap" ref={ref}>
      {showLegend && <ChartLegend items={series.map((s) => ({ label: s.label, color: chartColor(s.slot) }))} />}
      <svg
        className="g-chart"
        width={width}
        height={height}
        role="img"
        aria-label={aria['aria-label']}
        onPointerMove={onMove}
        onPointerLeave={() => setHover(null)}
      >
        {ticks.map((t) => (
          <g key={t}>
            <line className="grid" x1={PAD.left} x2={width - PAD.right} y1={y(t)} y2={y(t)} />
            <text x={PAD.left - 8} y={y(t) + 4} textAnchor="end">{formatValue(t)}</text>
          </g>
        ))}
        <line className="axis" x1={PAD.left} x2={width - PAD.right} y1={y(minV)} y2={y(minV)} />
        {xLabels.map((l, i) =>
          i % labelEvery === 0 ? <text key={`${l}-${i}`} x={x(i)} y={height - 6} textAnchor="middle">{l}</text> : null,
        )}
        {hover !== null && <line className="cross" x1={hx} x2={hx} y1={PAD.top} y2={y(minV)} />}
        {paths.map(({ s, d }) => (
          <path key={s.id} className="line" d={d} style={{ stroke: chartColor(s.slot) }} />
        ))}
        {hover !== null &&
          series.map((s) => {
            const v = s.values[hover];
            return v === null || v === undefined ? null : (
              <circle key={s.id} className="dot" cx={hx} cy={y(v)} r={4} style={{ fill: chartColor(s.slot) }} />
            );
          })}
      </svg>
      {hover !== null && hoverLabel !== undefined && (
        <div
          className="g-tooltip g-chart-tip"
          style={{ left: hx > width / 2 ? undefined : hx + 12, right: hx > width / 2 ? width - hx + 12 : undefined, top: PAD.top + (showLegend ? 24 : 0) }}
        >
          <TooltipContent
            title={formatX(hoverLabel, hover)}
            rows={series.flatMap((s) => {
              const v = s.values[hover];
              return v === null || v === undefined ? [] : [{ label: s.label, value: formatValue(v), color: chartColor(s.slot) }];
            })}
          />
        </div>
      )}
    </div>
  );
}

export interface BarListItem {
  id: string;
  label: string;
  value: number;
  /** Comparison value (prior period), drawn above in `chart-other`. */
  previous?: number;
}

export interface BarListProps {
  items: readonly BarListItem[];
  /** Accessible summary. */
  'aria-label': string;
  formatValue?: (value: number) => string;
  /** Legend labels when `previous` is used. */
  currentLabel?: string;
  previousLabel?: string;
  /** Fixed max; defaults to the largest value. */
  max?: number;
}

/** Ranked horizontal bars with optional prior-period comparison. Label and value are always visible. */
export function BarList({ items, formatValue = (v) => v.toLocaleString(), currentLabel = 'Current', previousLabel = 'Previous', max, ...aria }: BarListProps): ReactElement {
  const hasPrev = items.some((i) => i.previous !== undefined);
  const top = max ?? Math.max(1, ...items.flatMap((i) => [i.value, i.previous ?? 0]));
  const pct = (v: number): string => `${Math.max(0.5, (Math.max(0, v) / top) * 100).toFixed(2)}%`;
  return (
    <div className="g-barlist" role="list" aria-label={aria['aria-label']}>
      {items.map((i) => (
        <div key={i.id} role="listitem" className="g-barlist-row" title={`${i.label}: ${formatValue(i.value)}${i.previous !== undefined ? ` (${previousLabel} ${formatValue(i.previous)})` : ''}`}>
          <span className="g-barlist-label">{i.label}</span>
          <span className="g-barlist-bars">
            {i.previous !== undefined && <span className="g-barlist-bar" style={{ width: pct(i.previous), background: chartColor('other') }} />}
            <span className="g-barlist-bar" style={{ width: pct(i.value), background: chartColor(1) }} />
          </span>
          <span className="g-barlist-value g-num">{formatValue(i.value)}</span>
        </div>
      ))}
      {hasPrev && (
        <div style={{ marginTop: 'var(--space-2)' }}>
          <ChartLegend items={[{ label: previousLabel, color: chartColor('other') }, { label: currentLabel, color: chartColor(1) }]} />
        </div>
      )}
    </div>
  );
}

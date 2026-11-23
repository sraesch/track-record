import type { HTMLAttributes, ReactElement, ReactNode } from 'react';
import { cx } from '../utils/cx';
import { Icon } from './Icon';

export interface StatDelta {
  /** Formatted change, e.g. "12.4%" or "0.4 pt". */
  value: string;
  /** Which way the metric moved (drives the arrow). */
  direction: 'up' | 'down' | 'flat';
  /** Whether the move is good or bad (drives the colour). Churn going up is `negative`. */
  sentiment: 'positive' | 'negative' | 'neutral';
}

export interface StatTileProps extends HTMLAttributes<HTMLDivElement> {
  label: ReactNode;
  /** Pre-formatted headline value, e.g. "€4.82M". Put the exact value in `title`. */
  value: string;
  delta?: StatDelta;
  /** Comparison period, e.g. "vs prior 30 days". Always state it when there is a delta. */
  comparison?: string;
  /** Values for a sparkline, oldest first. */
  trend?: readonly number[];
}

export interface SparklineProps {
  values: readonly number[];
  height?: number;
  className?: string;
}

/** A single-series trend line with no axes, scaled to its container width. */
export function Sparkline({ values, height = 32, className }: SparklineProps): ReactElement | null {
  if (values.length < 2) return null;
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = max - min || 1;
  const w = 216;
  const d = values
    .map((v, i) => `${i === 0 ? 'M' : 'L'}${((i / (values.length - 1)) * w).toFixed(1)} ${(2 + (1 - (v - min) / span) * (height - 4)).toFixed(1)}`)
    .join(' ');
  return (
    <svg className={cx('g-spark', className)} viewBox={`0 0 ${w} ${height}`} preserveAspectRatio="none" style={{ height }} aria-hidden="true">
      <path d={d} vectorEffect="non-scaling-stroke" />
    </svg>
  );
}

const sentimentClass = { positive: 'g-delta-up', negative: 'g-delta-down', neutral: 'g-delta-flat' } as const;

/** One headline number with its comparison. */
export function StatTile({ label, value, delta, comparison, trend, className, ...rest }: StatTileProps): ReactElement {
  return (
    <div className={cx('g-stat', className)} {...rest}>
      <span className="g-stat-label">{label}</span>
      <span className="g-stat-value">{value}</span>
      {(delta !== undefined || comparison !== undefined) && (
        <span className="g-stat-meta">
          {delta !== undefined && (
            <span className={cx('g-delta', sentimentClass[delta.sentiment])}>
              {delta.direction !== 'flat' && <Icon name={delta.direction === 'up' ? 'arrow-up' : 'arrow-down'} size="sm" />}
              {delta.value}
            </span>
          )}
          {comparison}
        </span>
      )}
      {trend !== undefined && <Sparkline values={trend} />}
    </div>
  );
}

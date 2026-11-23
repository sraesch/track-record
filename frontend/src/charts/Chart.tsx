import { lazy, Suspense } from 'react'
import { useTranslation } from 'react-i18next'
import { ErrorBoundary } from '../pages/ErrorBoundary'
import { Alert, Button, Skeleton, VisuallyHidden } from '../ui'
import type { EChartProps } from './EChart'

// ECharts is large: it is loaded with the first chart a page shows, as its
// own chunk (ADR 0015, vite.config.ts).
const EChart = lazy(() =>
  import('./EChart').then((module) => ({ default: module.EChart })),
)

type Props = EChartProps & {
  /** What the chart shows, for screen readers. */
  label: string
}

/**
 * A chart (ADR 0015), described for screen readers by `label`. Shows a
 * placeholder while the chart code loads, and an error with a reload
 * button instead of breaking the page if it cannot be loaded (e.g. after a
 * new release replaced it).
 */
export function Chart({ label, ...chart }: Props) {
  return (
    <div style={{ position: 'relative' }}>
      <VisuallyHidden>{label}</VisuallyHidden>
      <ErrorBoundary fallback={() => <ChartFailed />}>
        <Suspense fallback={<ChartPlaceholder height={chart.height} />}>
          <EChart {...chart} />
        </Suspense>
      </ErrorBoundary>
    </div>
  )
}

/** Takes the place of a chart while it or its data loads. */
export function ChartPlaceholder({ height }: { height: number }) {
  const { t } = useTranslation()
  return <Skeleton height={height} label={t('common.loading')} />
}

function ChartFailed() {
  const { t } = useTranslation()
  return (
    <Alert
      tone="error"
      action={
        <Button
          variant="ghost"
          size="sm"
          onClick={() => window.location.reload()}
        >
          {t('charts.reload')}
        </Button>
      }
    >
      {t('charts.failed')}
    </Alert>
  )
}

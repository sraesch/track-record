import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import type { SampleDetail } from '../../api/client'
import { formatNumber } from '../../format'
import { Section } from '../../pages/Section'
import { Table } from '../../ui'
import { byRepetition, warmupRowClass } from './samples'
import { SeriesCharts } from './SeriesCharts'
import { WarmupChip } from './WarmupChip'
import './case.css'

/**
 * The series of every sample (at most one per metric and sample): a chart
 * per metric with a line per sample, then what is measured over which x
 * dimension, in which units, and how many points.
 */
export function SampleSeries({
  runId,
  samples,
}: {
  runId: string
  samples: readonly SampleDetail[]
}) {
  const { t } = useTranslation()
  const rows = useMemo(
    () =>
      byRepetition(samples).flatMap((sample) =>
        [...sample.series]
          .sort((a, b) => a.metric.localeCompare(b.metric))
          .map((series) => ({ sample, series })),
      ),
    [samples],
  )

  return (
    <Section
      title={t('caseExecution.series.title')}
      flush
      actions={
        <span className="small ui-secondary">
          {t('caseExecution.series.count', { count: rows.length })}
        </span>
      }
    >
      {rows.length === 0 ? (
        <p className="ui-text ui-secondary tr-case-flush-text">
          {t('caseExecution.series.empty')}
        </p>
      ) : (
        <>
          <SeriesCharts runId={runId} samples={samples} />
          <Table aria-label={t('caseExecution.series.title')}>
            <thead>
              <tr>
                <th>{t('caseExecution.repetition')}</th>
                <th>{t('caseExecution.series.metric')}</th>
                <th>{t('caseExecution.series.unit')}</th>
                <th>{t('caseExecution.series.x')}</th>
                <th className="g-r">{t('caseExecution.series.points')}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map(({ sample, series }) => (
                <tr
                  key={series.id}
                  className={sample.is_warmup ? warmupRowClass : undefined}
                >
                  <td>
                    {sample.repetition}
                    {sample.is_warmup && <WarmupChip />}
                  </td>
                  <td className="ui-wrap-cell">{series.metric}</td>
                  <td>{series.unit || t('common.none')}</td>
                  <td className="ui-wrap-cell">
                    {series.x_unit
                      ? t('caseExecution.series.xWithUnit', {
                          dimension: series.x_dimension,
                          unit: series.x_unit,
                        })
                      : series.x_dimension}
                  </td>
                  <td className="g-r">{formatNumber(series.point_count)}</td>
                </tr>
              ))}
            </tbody>
          </Table>
        </>
      )}
    </Section>
  )
}

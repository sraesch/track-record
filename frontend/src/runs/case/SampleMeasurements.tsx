import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import type { SampleDetail } from '../../api/client'
import { formatNumber } from '../../format'
import { Section } from '../../pages/Section'
import { Table } from '../../ui'
import { byRepetition, warmupRowClass } from './samples'
import { WarmupChip } from './WarmupChip'
import './case.css'

type Column = SampleDetail['measurements'][number]

/**
 * The scalar measurements of every sample as uploaded: one row per sample,
 * one column per metric. Nothing is aggregated here.
 */
export function SampleMeasurements({
  samples,
}: {
  samples: readonly SampleDetail[]
}) {
  const { t } = useTranslation()
  const rows = useMemo(() => byRepetition(samples), [samples])
  const columns = useMemo(() => {
    const byId = new Map<number, Column>()
    for (const sample of samples) {
      for (const m of sample.measurements) byId.set(m.metric_id, m)
    }
    return [...byId.values()].sort((a, b) => a.metric.localeCompare(b.metric))
  }, [samples])
  const warmup = samples.filter((s) => s.is_warmup).length

  return (
    <Section
      title={t('caseExecution.measurements.title')}
      flush
      actions={
        <span className="small ui-secondary">
          {warmup > 0
            ? t('caseExecution.measurements.countWithWarmup', {
                count: samples.length,
                warmup,
              })
            : t('caseExecution.measurements.count', { count: samples.length })}
        </span>
      }
    >
      {columns.length === 0 ? (
        <p className="ui-text ui-secondary tr-case-flush-text">
          {t('caseExecution.measurements.empty')}
        </p>
      ) : (
        <Table aria-label={t('caseExecution.measurements.title')}>
          <thead>
            <tr>
              <th className="tr-case-metric-head">
                {t('caseExecution.repetition')}
              </th>
              {columns.map((column) => (
                <th key={column.metric_id} className="g-r tr-case-metric-head">
                  <span>{column.metric}</span>
                  {/* An empty line without a unit keeps the names aligned. */}
                  <span className="tr-case-metric-unit">
                    {column.unit || ' '}
                  </span>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((sample) => {
              const values = new Map(
                sample.measurements.map((m) => [m.metric_id, m.value]),
              )
              return (
                <tr
                  key={sample.id}
                  className={sample.is_warmup ? warmupRowClass : undefined}
                >
                  <td>
                    {sample.repetition}
                    {sample.is_warmup && <WarmupChip />}
                  </td>
                  {columns.map((column) => {
                    const value = values.get(column.metric_id)
                    return (
                      <td key={column.metric_id} className="g-r">
                        {value === undefined
                          ? t('common.none')
                          : formatNumber(value)}
                      </td>
                    )
                  })}
                </tr>
              )
            })}
          </tbody>
        </Table>
      )}
    </Section>
  )
}

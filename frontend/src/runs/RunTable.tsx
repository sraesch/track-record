import { cx } from '@graphite/react'
import { useTranslation } from 'react-i18next'
import type { RunSummary } from '../api/client'
import {
  formatDateTime,
  formatDuration,
  formatTimestamp,
  shortCommit,
} from '../format'
import { useRowLink } from '../pages/useRowLink'
import { Chip, Stack, Table, TextLink, Tooltip } from '../ui'
import { runPath } from './paths'
import { InvalidChip, RunStatusChip } from './RunStatusChip'
import './runs.css'

type Props = {
  runs: readonly RunSummary[]
  /** Called when a tag chip is clicked, to filter by that tag. */
  onTagClick: (tag: string) => void
}

/** Runs as a table; a row opens the run (FR-21: /runs/:runId). */
export function RunTable({ runs, onTagClick }: Props) {
  const { t } = useTranslation()
  const rowLink = useRowLink()

  return (
    <Table framed aria-label={t('nav.items.runs')}>
      <thead>
        <tr>
          <th>{t('runs.columns.status')}</th>
          <th>{t('runs.columns.started')}</th>
          <th>{t('runs.columns.duration')}</th>
          <th>{t('runs.columns.suite')}</th>
          <th>{t('runs.columns.sutVersion')}</th>
          <th>{t('runs.columns.trigger')}</th>
          <th>{t('runs.columns.tags')}</th>
        </tr>
      </thead>
      <tbody>
        {runs.map((run) => {
          const href = runPath(run.id)
          const started = formatDateTime(run.started_at)
          return (
            <tr
              key={run.id}
              className={cx('ui-row-link', !run.is_valid && 'tr-runs-dim')}
              onClick={rowLink(href)}
            >
              <td>
                <Stack row gap={1}>
                  <RunStatusChip status={run.status} />
                  {!run.is_valid && <InvalidChip reason={run.invalid_reason} />}
                </Stack>
              </td>
              <td>
                <Tooltip title={formatTimestamp(run.started_at)}>
                  <TextLink to={href} aria-label={t('runs.open', { started })}>
                    {started}
                  </TextLink>
                </Tooltip>
              </td>
              <td>
                {run.ended_at
                  ? formatDuration(run.started_at, run.ended_at)
                  : run.status === 'running'
                    ? t('runs.stillRunning')
                    : t('common.none')}
              </td>
              <td>
                {run.suite.name}{' '}
                <span className="ui-secondary">
                  {t('runs.version', { version: run.suite.version })}
                </span>
              </td>
              <td>
                <SutVersion version={run.sut_version} />
              </td>
              <td>{t(`runs.trigger.${run.triggered_by}`)}</td>
              <td>
                <Stack row gap={1}>
                  {run.tags.map((tag) => (
                    <Chip
                      key={tag}
                      aria-label={t('runs.filterByTag', { tag })}
                      onClick={() => onTagClick(tag)}
                    >
                      {tag}
                    </Chip>
                  ))}
                </Stack>
              </td>
            </tr>
          )
        })}
      </tbody>
    </Table>
  )
}

function SutVersion({ version }: { version: RunSummary['sut_version'] }) {
  return (
    <Stack row gap={2} align="baseline">
      {version.branch && <span>{version.branch}</span>}
      <Tooltip title={version.commit}>
        <code className="ui-code ui-secondary">
          {shortCommit(version.commit)}
        </code>
      </Tooltip>
      {version.tag && <Chip outlined>{version.tag}</Chip>}
      {version.build_config && (
        <span className="ui-secondary">{version.build_config}</span>
      )}
    </Stack>
  )
}

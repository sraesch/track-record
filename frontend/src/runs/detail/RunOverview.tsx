import { useTranslation } from 'react-i18next'
import type { RunDetail } from '../../api/client'
import { formatDuration, formatNumber, shortCommit } from '../../format'
import { CopyButton } from '../../pages/CopyButton'
import { type Fact, Facts } from '../../pages/Facts'
import { Section } from '../../pages/Section'
import { Time } from '../../pages/Time'
import { suiteVersionHref } from '../../suites/paths'
import { Chip, Stack, TextLink, Tooltip } from '../../ui'
import { InvalidChip, RunStatusChip } from '../RunStatusChip'

/** What a run is: status, times, suite and SUT version. */
export function RunOverview({ run }: { run: RunDetail }) {
  const { t } = useTranslation()
  const { sut_version: sut } = run

  const ended = run.ended_at ? (
    <>
      <Time iso={run.ended_at} />{' '}
      <span className="ui-secondary">
        {t('runDetail.overview.durationOf', {
          duration: formatDuration(run.started_at, run.ended_at),
        })}
      </span>
    </>
  ) : run.status === 'running' ? (
    t('runs.stillRunning')
  ) : (
    t('common.none')
  )

  const facts: Fact[] = [
    {
      label: t('runDetail.overview.status'),
      value: (
        <Stack row wrap gap={1}>
          <RunStatusChip status={run.status} />
          {!run.is_valid && <InvalidChip reason={run.invalid_reason} />}
        </Stack>
      ),
    },
    {
      label: t('runDetail.overview.trigger'),
      value: t(`runs.trigger.${run.triggered_by}`),
    },
    {
      label: t('runDetail.overview.started'),
      value: <Time iso={run.started_at} />,
    },
    { label: t('runDetail.overview.ended'), value: ended },
    {
      label: t('runDetail.overview.suite'),
      value: (
        <>
          <TextLink to={suiteVersionHref(run.suite.name, run.suite.version)}>
            {run.suite.name}
          </TextLink>{' '}
          <span className="ui-secondary">
            {t('runs.version', { version: run.suite.version })}
          </span>
        </>
      ),
    },
    {
      label: t('runDetail.overview.caseExecutions'),
      value: formatNumber(run.case_execution_count),
    },
    {
      label: t('runDetail.overview.commit'),
      value: (
        <span className="ui-inline-row">
          <Tooltip title={sut.commit}>
            <code className="ui-code">{shortCommit(sut.commit)}</code>
          </Tooltip>
          <CopyButton
            text={sut.commit}
            label={t('runDetail.overview.copyCommit')}
          />
        </span>
      ),
    },
    {
      label: t('runDetail.overview.branch'),
      value: sut.branch ?? t('common.none'),
    },
    {
      label: t('runDetail.overview.releaseTag'),
      value: sut.tag ? <Chip outlined>{sut.tag}</Chip> : t('common.none'),
    },
    {
      label: t('runDetail.overview.buildConfig'),
      value: sut.build_config ?? t('common.none'),
    },
    {
      label: t('runDetail.overview.lastUpload'),
      value: <Time iso={run.last_upload_at} />,
    },
    {
      label: t('runDetail.overview.created'),
      value: <Time iso={run.created_at} />,
    },
    {
      label: t('runDetail.overview.runId'),
      value: (
        <span className="ui-inline-row">
          <code className="ui-code">{run.id}</code>
          <CopyButton text={run.id} label={t('runDetail.overview.copyRunId')} />
        </span>
      ),
    },
  ]

  return (
    <Section title={t('runDetail.overview.title')}>
      <Facts items={facts} />
    </Section>
  )
}

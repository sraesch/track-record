import { useTranslation } from 'react-i18next'
import type { CaseExecutionDetail, RunDetail } from '../../api/client'
import { formatDateTime, formatDuration, formatNumber } from '../../format'
import { CopyButton } from '../../pages/CopyButton'
import { type Fact, Facts } from '../../pages/Facts'
import { Section } from '../../pages/Section'
import { Time } from '../../pages/Time'
import { testCaseHref } from '../../suites/paths'
import { TextLink } from '../../ui'
import { runPath } from '../paths'
import { InvalidChip, RunStatusChip } from '../RunStatusChip'
import { Aliased, AssetList } from '../TestCaseParts'

type Props = {
  caseExecution: CaseExecutionDetail
  /** Not there while it loads or if it failed to load. */
  run: RunDetail | undefined
}

/** What a case execution is: its run, test case, device, times and samples. */
export function CaseOverview({ caseExecution: ce, run }: Props) {
  const { t } = useTranslation()
  const tc = ce.test_case
  const warmup = ce.samples.filter((s) => s.is_warmup).length

  const ended = ce.ended_at ? (
    <>
      <Time iso={ce.ended_at} />
      {ce.started_at && (
        <>
          {' '}
          <span className="ui-secondary">
            {t('runDetail.overview.durationOf', {
              duration: formatDuration(ce.started_at, ce.ended_at),
            })}
          </span>
        </>
      )}
    </>
  ) : (
    t('common.none')
  )

  const facts: Fact[] = [
    {
      label: t('caseExecution.overview.run'),
      value: (
        <span className="ui-inline-row" style={{ flexWrap: 'wrap' }}>
          <TextLink to={runPath(ce.run_id)}>
            {run
              ? t('caseExecution.overview.runName', {
                  suite: run.suite.name,
                  started: formatDateTime(run.started_at),
                })
              : t('caseExecution.overview.openRun')}
          </TextLink>
          {run && <RunStatusChip status={run.status} />}
          {run && !run.is_valid && <InvalidChip reason={run.invalid_reason} />}
        </span>
      ),
    },
    {
      label: t('caseExecution.overview.scenario'),
      value: (
        <>
          <TextLink to={testCaseHref(tc)}>{tc.scenario}</TextLink>{' '}
          <span className="ui-secondary">
            {t('runs.version', { version: tc.scenario_version })}
          </span>
        </>
      ),
    },
    {
      label: t('caseExecution.overview.device'),
      value: (
        <Aliased
          shown={ce.client_device.resolved_name}
          uploaded={ce.client_device.name}
        />
      ),
    },
    {
      label: t('caseExecution.overview.samples'),
      value: (
        <>
          {formatNumber(ce.sample_count)}
          {warmup > 0 && (
            <>
              {' '}
              <span className="ui-secondary">
                {t('caseExecution.overview.warmup', { count: warmup })}
              </span>
            </>
          )}
        </>
      ),
    },
    {
      label: t('caseExecution.overview.started'),
      value: ce.started_at ? <Time iso={ce.started_at} /> : t('common.none'),
    },
    { label: t('caseExecution.overview.ended'), value: ended },
    {
      label: t('caseExecution.overview.assets'),
      value: <AssetList assets={tc.assets} />,
    },
    {
      label: t('caseExecution.overview.created'),
      value: <Time iso={ce.created_at} />,
    },
    {
      label: t('caseExecution.overview.id'),
      value: (
        <span className="ui-inline-row">
          <code className="ui-code">{ce.id}</code>
          <CopyButton text={ce.id} label={t('caseExecution.overview.copyId')} />
        </span>
      ),
    },
  ]

  return (
    <Section title={t('caseExecution.overview.title')}>
      <Facts items={facts} />
    </Section>
  )
}

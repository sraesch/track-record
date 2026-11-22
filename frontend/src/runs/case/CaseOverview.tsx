import Box from '@mui/material/Box'
import Chip from '@mui/material/Chip'
import Link from '@mui/material/Link'
import Typography from '@mui/material/Typography'
import { useTranslation } from 'react-i18next'
import { Link as RouterLink } from 'react-router'
import type { CaseExecutionDetail, RunDetail } from '../../api/client'
import { formatDateTime, formatDuration, formatNumber } from '../../format'
import { CopyButton } from '../../pages/CopyButton'
import { type Fact, Facts } from '../../pages/Facts'
import { Section } from '../../pages/Section'
import { codeSx, inlineRow } from '../../pages/styles'
import { Time } from '../../pages/Time'
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
          <Typography component="span" variant="body2" color="text.secondary">
            {t('runDetail.overview.durationOf', {
              duration: formatDuration(ce.started_at, ce.ended_at),
            })}
          </Typography>
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
        <Box component="span" sx={{ ...inlineRow, flexWrap: 'wrap' }}>
          <Link component={RouterLink} to={runPath(ce.run_id)} underline="hover">
            {run
              ? t('caseExecution.overview.runName', {
                  suite: run.suite.name,
                  started: formatDateTime(run.started_at),
                })
              : t('caseExecution.overview.openRun')}
          </Link>
          {run && <RunStatusChip status={run.status} />}
          {run && !run.is_valid && <InvalidChip reason={run.invalid_reason} />}
        </Box>
      ),
    },
    {
      label: t('caseExecution.overview.scenario'),
      value: (
        <>
          {tc.scenario}{' '}
          <Typography component="span" variant="body2" color="text.secondary">
            {t('runs.version', { version: tc.scenario_version })}
          </Typography>
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
              <Typography component="span" variant="body2" color="text.secondary">
                {t('caseExecution.overview.warmup', { count: warmup })}
              </Typography>
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
      label: t('caseExecution.overview.tags'),
      value:
        tc.tags.length === 0 ? (
          t('common.none')
        ) : (
          <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5 }}>
            {tc.tags.map((tag) => (
              <Chip key={tag} size="small" variant="outlined" label={tag} />
            ))}
          </Box>
        ),
    },
    {
      label: t('caseExecution.overview.created'),
      value: <Time iso={ce.created_at} />,
    },
    {
      label: t('caseExecution.overview.id'),
      value: (
        <Box component="span" sx={inlineRow}>
          <Typography component="code" variant="body2" sx={codeSx}>
            {ce.id}
          </Typography>
          <CopyButton text={ce.id} label={t('caseExecution.overview.copyId')} />
        </Box>
      ),
    },
  ]

  return (
    <Section title={t('caseExecution.overview.title')}>
      <Facts items={facts} />
    </Section>
  )
}

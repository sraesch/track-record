import Box from '@mui/material/Box'
import Chip from '@mui/material/Chip'
import Tooltip from '@mui/material/Tooltip'
import Typography from '@mui/material/Typography'
import { useTranslation } from 'react-i18next'
import type { RunDetail } from '../../api/client'
import { formatDuration, formatNumber, shortCommit } from '../../format'
import { CopyButton } from '../../pages/CopyButton'
import { type Fact, Facts } from '../../pages/Facts'
import { Section } from '../../pages/Section'
import { Time } from '../../pages/Time'
import { codeSx, inlineRow } from '../../pages/styles'
import { InvalidChip, RunStatusChip } from '../RunStatusChip'

/** What a run is: status, times, suite and SUT version. */
export function RunOverview({ run }: { run: RunDetail }) {
  const { t } = useTranslation()
  const { sut_version: sut } = run

  const ended = run.ended_at ? (
    <>
      <Time iso={run.ended_at} />{' '}
      <Typography component="span" variant="body2" color="text.secondary">
        {t('runDetail.overview.durationOf', {
          duration: formatDuration(run.started_at, run.ended_at),
        })}
      </Typography>
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
        <Box sx={{ display: 'flex', gap: 0.5, flexWrap: 'wrap' }}>
          <RunStatusChip status={run.status} />
          {!run.is_valid && <InvalidChip reason={run.invalid_reason} />}
        </Box>
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
          {run.suite.name}{' '}
          <Typography component="span" variant="body2" color="text.secondary">
            {t('runs.version', { version: run.suite.version })}
          </Typography>
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
        <Box component="span" sx={inlineRow}>
          <Tooltip describeChild title={sut.commit}>
            <Typography component="code" variant="body2" sx={codeSx}>
              {shortCommit(sut.commit)}
            </Typography>
          </Tooltip>
          <CopyButton
            text={sut.commit}
            label={t('runDetail.overview.copyCommit')}
          />
        </Box>
      ),
    },
    {
      label: t('runDetail.overview.branch'),
      value: sut.branch ?? t('common.none'),
    },
    {
      label: t('runDetail.overview.releaseTag'),
      value: sut.tag ? (
        <Chip size="small" variant="outlined" label={sut.tag} />
      ) : (
        t('common.none')
      ),
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
        <Box component="span" sx={inlineRow}>
          <Typography component="code" variant="body2" sx={codeSx}>
            {run.id}
          </Typography>
          <CopyButton text={run.id} label={t('runDetail.overview.copyRunId')} />
        </Box>
      ),
    },
  ]

  return (
    <Section title={t('runDetail.overview.title')}>
      <Facts items={facts} />
    </Section>
  )
}

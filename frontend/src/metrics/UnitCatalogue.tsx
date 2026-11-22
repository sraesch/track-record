import Box from '@mui/material/Box'
import Chip from '@mui/material/Chip'
import Table from '@mui/material/Table'
import TableBody from '@mui/material/TableBody'
import TableCell from '@mui/material/TableCell'
import TableHead from '@mui/material/TableHead'
import TableRow from '@mui/material/TableRow'
import Typography from '@mui/material/Typography'
import { useTranslation } from 'react-i18next'
import { Link as RouterLink } from 'react-router'
import { formatNumber } from '../format'
import { Section } from '../pages/Section'
import { metricsViewToParams, type QuantityGroup } from './metricModel'

type Props = {
  groups: readonly QuantityGroup[]
  /** Custom units used by metrics (not converted), shown first. */
  custom: readonly { symbol: string; metrics: string[] }[]
  /** Metric names per unit symbol (latest versions). */
  usedBy: ReadonlyMap<string, string[]>
}

// Sections clip their content; a table wider than a phone scrolls instead.
const scroll = { overflowX: 'auto' } as const

const grid = {
  display: 'grid',
  gap: 2,
  gridTemplateColumns: 'repeat(auto-fill, minmax(min(100%, 440px), 1fr))',
  alignItems: 'start',
}

/**
 * The predefined units (FR-13a), one section per quantity. Values convert
 * between the units of one quantity through its base unit.
 */
export function UnitCatalogue({ groups, custom, usedBy }: Props) {
  const { t } = useTranslation()
  return (
    <Box sx={grid}>
      {custom.length > 0 && (
        <Section title={t('metrics.units.customTitle')} flush>
          <Typography
            variant="body2"
            color="text.secondary"
            sx={{ px: 2, pb: 1 }}
          >
            {t('metrics.units.customText')}
          </Typography>
          <Box sx={scroll}>
            <Table size="small" aria-label={t('metrics.units.customTitle')}>
              <TableHead>
                <TableRow>
                  <TableCell>{t('metrics.units.columns.symbol')}</TableCell>
                  <TableCell>{t('metrics.units.columns.usedBy')}</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {custom.map((unit) => (
                  <TableRow key={unit.symbol}>
                    <TableCell>
                      <Symbol symbol={unit.symbol} />
                    </TableCell>
                    <TableCell>
                      <MetricLinks names={unit.metrics} />
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </Box>
        </Section>
      )}

      {groups.map((group) => (
        <Section
          key={group.quantity}
          title={capitalize(group.quantity)}
          flush
          actions={
            group.base && (
              <Typography variant="body2" color="text.secondary">
                {t('metrics.units.baseUnit', {
                  unit: group.base.symbol || t('metrics.unit.dimensionless'),
                })}
              </Typography>
            )
          }
        >
          <Box sx={scroll}>
            <Table size="small" aria-label={capitalize(group.quantity)}>
              <TableHead>
                <TableRow>
                  <TableCell>{t('metrics.units.columns.symbol')}</TableCell>
                  <TableCell>{t('metrics.units.columns.name')}</TableCell>
                  <TableCell align="right">
                    {t('metrics.units.columns.inBase')}
                  </TableCell>
                  <TableCell>{t('metrics.units.columns.usedBy')}</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {group.units.map((unit) => (
                  <TableRow key={unit.symbol}>
                    <TableCell sx={{ whiteSpace: 'nowrap' }}>
                      <Symbol symbol={unit.symbol} />
                    </TableCell>
                    <TableCell>{unit.name}</TableCell>
                    <TableCell
                      align="right"
                      sx={{
                        whiteSpace: 'nowrap',
                        fontVariantNumeric: 'tabular-nums',
                      }}
                    >
                      {unit === group.base ? (
                        <Chip
                          size="small"
                          variant="outlined"
                          label={t('metrics.units.base')}
                        />
                      ) : group.base ? (
                        t('common.valueWithUnit', {
                          value: formatNumber(unit.to_base),
                          unit: group.base.symbol,
                        }).trim()
                      ) : (
                        formatNumber(unit.to_base)
                      )}
                    </TableCell>
                    <TableCell>
                      <MetricLinks names={usedBy.get(unit.symbol) ?? []} />
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </Box>
        </Section>
      ))}
    </Box>
  )
}

function Symbol({ symbol }: { symbol: string }) {
  const { t } = useTranslation()
  return symbol === '' ? (
    <Typography component="span" variant="body2" color="text.secondary">
      {t('metrics.unit.dimensionless')}
    </Typography>
  ) : (
    <Box component="code" sx={{ fontFamily: 'monospace' }}>
      {symbol}
    </Box>
  )
}

/** The metrics using a unit; each opens the metric list searched for it. */
function MetricLinks({ names }: { names: readonly string[] }) {
  const { t } = useTranslation()
  if (names.length === 0) {
    return (
      <Typography component="span" variant="body2" color="text.secondary">
        {t('common.none')}
      </Typography>
    )
  }
  return (
    <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5 }}>
      {names.map((name) => (
        <Chip
          key={name}
          size="small"
          clickable
          component={RouterLink}
          to={`/metrics?${metricsViewToParams({ tab: 'metrics', search: name })}`}
          label={name}
          aria-label={t('metrics.units.showMetric', { name })}
        />
      ))}
    </Box>
  )
}

/** Quantities come from the catalogue in lower case ("data size"). */
const capitalize = (text: string) =>
  text.charAt(0).toLocaleUpperCase() + text.slice(1)

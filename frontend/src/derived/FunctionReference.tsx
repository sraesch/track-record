import ExpandMoreIcon from '@mui/icons-material/ExpandMore'
import Accordion from '@mui/material/Accordion'
import AccordionDetails from '@mui/material/AccordionDetails'
import AccordionSummary from '@mui/material/AccordionSummary'
import Box from '@mui/material/Box'
import Table from '@mui/material/Table'
import TableBody from '@mui/material/TableBody'
import TableCell from '@mui/material/TableCell'
import TableRow from '@mui/material/TableRow'
import Typography from '@mui/material/Typography'
import { useTranslation } from 'react-i18next'
import { dslFunctions } from '../dsl/syntax'

const syntaxExamples = [
  ['load_time', 'metric'],
  ['series.fps', 'series'],
  ['derived.throughput', 'derived'],
  ['`load time (cold)`', 'quoted'],
  ['500[ms]', 'unit'],
  ['+ - * / ^', 'operators'],
] as const

/** The DSL at a glance (ADR 0009): how names are written and the functions. */
export function FunctionReference() {
  const { t } = useTranslation()
  return (
    <Accordion
      variant="outlined"
      disableGutters
      slotProps={{ transition: { unmountOnExit: true } }}
      sx={{ '&::before': { display: 'none' } }}
    >
      <AccordionSummary expandIcon={<ExpandMoreIcon />}>
        <Typography variant="body2">{t('dsl.reference.title')}</Typography>
      </AccordionSummary>
      <AccordionDetails sx={{ pt: 0 }}>
        <Table size="small" aria-label={t('dsl.reference.syntax')}>
          <TableBody>
            {syntaxExamples.map(([example, key]) => (
              <Row key={key} code={example} text={t(`dsl.reference.${key}`)} />
            ))}
          </TableBody>
        </Table>
        <Typography variant="subtitle2" component="h3" sx={{ mt: 2, mb: 0.5 }}>
          {t('dsl.reference.functions')}
        </Typography>
        <Table size="small" aria-label={t('dsl.reference.functions')}>
          <TableBody>
            {dslFunctions.map((f) => (
              <Row
                key={f.name}
                code={f.signature}
                text={t(`dsl.functions.${f.name}`)}
              />
            ))}
          </TableBody>
        </Table>
        <Typography variant="body2" color="text.secondary" sx={{ mt: 1.5 }}>
          {t('dsl.reference.noValue')}
        </Typography>
      </AccordionDetails>
    </Accordion>
  )
}

function Row({ code, text }: { code: string; text: string }) {
  return (
    <TableRow>
      <TableCell sx={{ width: '40%', verticalAlign: 'top' }}>
        <Box
          component="code"
          sx={{ fontFamily: 'monospace', overflowWrap: 'anywhere' }}
        >
          {code}
        </Box>
      </TableCell>
      <TableCell sx={{ verticalAlign: 'top' }}>{text}</TableCell>
    </TableRow>
  )
}

import Typography from '@mui/material/Typography'
import { useTranslation } from 'react-i18next'
import { formatJsonValue } from '../../format'
import { Facts } from '../../pages/Facts'
import { Section } from '../../pages/Section'
import { flattenJson, jsonObject } from './json'

/**
 * The shared environment of a run (e.g. the backend it ran against), as
 * uploaded by the harness. Nested details are shown with dotted keys.
 */
export function EnvironmentDetails({ details }: { details: object }) {
  const { t } = useTranslation()
  const entries = flattenJson(jsonObject(details))
  return (
    <Section title={t('runDetail.environment.title')}>
      {entries.length === 0 ? (
        <Typography variant="body2" color="text.secondary">
          {t('runDetail.environment.empty')}
        </Typography>
      ) : (
        <Facts
          items={entries.map(([key, value]) => ({
            label: key,
            value: formatJsonValue(value),
            code: true,
          }))}
        />
      )}
    </Section>
  )
}

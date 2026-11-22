import Typography from '@mui/material/Typography'
import { formatJsonValue } from '../../format'
import { Facts } from '../../pages/Facts'
import { Section } from '../../pages/Section'
import { flattenJson, jsonObject } from './json'

type Props = {
  title: string
  /** Shown when there are no details. */
  emptyText: string
  details: object
}

/**
 * Details as uploaded by the harness, e.g. the shared environment of a run
 * or the client device of a case execution. Nested details are shown with
 * dotted keys.
 */
export function JsonDetails({ title, emptyText, details }: Props) {
  const entries = flattenJson(jsonObject(details))
  return (
    <Section title={title}>
      {entries.length === 0 ? (
        <Typography variant="body2" color="text.secondary">
          {emptyText}
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

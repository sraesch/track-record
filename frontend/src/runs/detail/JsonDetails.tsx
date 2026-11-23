import type { ReactNode } from 'react'
import { formatJsonValue } from '../../format'
import { Facts } from '../../pages/Facts'
import { Section } from '../../pages/Section'
import { flattenJson, jsonObject } from './json'

type Props = {
  title: string
  /** Shown when there are no details. */
  emptyText: string
  details: object
  /** Shown to the right of the title. */
  actions?: ReactNode
}

/**
 * Details as uploaded by the harness, e.g. the shared environment of a run
 * or the client device of a case execution. Nested details are shown with
 * dotted keys.
 */
export function JsonDetails({ title, emptyText, details, actions }: Props) {
  const entries = flattenJson(jsonObject(details))
  return (
    <Section title={title} actions={actions}>
      {entries.length === 0 ? (
        <p className="ui-text ui-secondary">{emptyText}</p>
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

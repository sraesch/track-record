import { useTranslation } from 'react-i18next'
import { dslFunctions } from '../dsl/syntax'
import { Disclosure, Table } from '../ui'
import './derived.css'

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
    <Disclosure summary={t('dsl.reference.title')}>
      <div className="tr-derived-reference">
        <Table
          framed
          className="tr-derived-reference-table"
          aria-label={t('dsl.reference.syntax')}
        >
          <tbody>
            {syntaxExamples.map(([example, key]) => (
              <Row key={key} code={example} text={t(`dsl.reference.${key}`)} />
            ))}
          </tbody>
        </Table>
        <h4
          className="body-medium ui-heading"
          style={{ marginTop: 'var(--space-2)' }}
        >
          {t('dsl.reference.functions')}
        </h4>
        <Table
          framed
          className="tr-derived-reference-table"
          aria-label={t('dsl.reference.functions')}
        >
          <tbody>
            {dslFunctions.map((f) => (
              <Row
                key={f.name}
                code={f.signature}
                text={t(`dsl.functions.${f.name}`)}
              />
            ))}
          </tbody>
        </Table>
        <p className="ui-text ui-secondary">{t('dsl.reference.noValue')}</p>
      </div>
    </Disclosure>
  )
}

function Row({ code, text }: { code: string; text: string }) {
  return (
    <tr>
      <td>
        <code className="ui-code">{code}</code>
      </td>
      <td>{text}</td>
    </tr>
  )
}

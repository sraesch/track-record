import { useTranslation } from 'react-i18next'
import type { TestCaseSummary } from '../../api/client'
import { Facts } from '../../pages/Facts'
import { Section } from '../../pages/Section'
import { useParameterValues } from '../parameterValues'
import { Aliased } from '../TestCaseParts'

/**
 * The parameters of the test case after aliases (FR-14), with their units
 * (FR-13a); an aliased value shows the uploaded one in its tooltip.
 */
export function CaseParameters({ testCase }: { testCase: TestCaseSummary }) {
  const { t } = useTranslation()
  const values = useParameterValues(testCase)
  return (
    <Section title={t('caseExecution.parameters.title')}>
      {values.length === 0 ? (
        <p className="ui-text ui-secondary">
          {t('caseExecution.parameters.empty')}
        </p>
      ) : (
        <Facts
          items={values.map(({ name, label, uploaded }) => ({
            label: name,
            value: <Aliased shown={label} uploaded={uploaded} />,
            code: true,
          }))}
        />
      )}
    </Section>
  )
}

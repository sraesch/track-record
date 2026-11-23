import { useTranslation } from 'react-i18next'
import type { TestCaseSummary } from '../api/client'
import { Chip, Stack, Tooltip } from '../ui'
import { useParameterValues } from './parameterValues'
import './runs.css'

// Parts of a test case (scenario version, assets, parameters) and its
// client device, shown in the case execution list and on its own page.

/** Parameters as chips, `name=value unit`. */
export function Parameters({ testCase }: { testCase: TestCaseSummary }) {
  const { t } = useTranslation()
  const values = useParameterValues(testCase)
  if (values.length === 0) return t('common.none')

  return (
    <Stack row wrap gap={1}>
      {values.map(({ name, label, uploaded }) => {
        const chip = (
          <Chip outlined>
            {t('runDetail.cases.parameter', { name, value: label })}
          </Chip>
        )
        return uploaded !== undefined ? (
          <Tooltip
            key={name}
            title={t('runDetail.cases.uploadedAs', { value: uploaded })}
          >
            {chip}
          </Tooltip>
        ) : (
          <span key={name}>{chip}</span>
        )
      })}
    </Stack>
  )
}

/** A name or value after aliases; the uploaded one in the tooltip if it differs. */
export function Aliased({
  shown,
  uploaded,
}: {
  shown: string
  uploaded: string | undefined
}) {
  const { t } = useTranslation()
  if (uploaded === undefined || shown === uploaded) return shown
  return (
    <Tooltip title={t('runDetail.cases.uploadedAs', { value: uploaded })}>
      <span className="tr-runs-aliased">{shown}</span>
    </Tooltip>
  )
}

/** The assets of a test case, one per line, the content hash in the tooltip. */
export function AssetList({ assets }: { assets: TestCaseSummary['assets'] }) {
  const { t } = useTranslation()
  if (assets.length === 0) return t('common.none')
  return assets.map((asset) => (
    <span key={asset.id} className="tr-runs-asset">
      <Tooltip title={asset.content_hash ?? ''}>
        <span>
          {asset.version
            ? t('runDetail.cases.assetVersion', {
                name: asset.name,
                version: asset.version,
              })
            : asset.name}
        </span>
      </Tooltip>
    </span>
  ))
}

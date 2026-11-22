import Box from '@mui/material/Box'
import Chip from '@mui/material/Chip'
import Tooltip from '@mui/material/Tooltip'
import { useTranslation } from 'react-i18next'
import type { TestCaseSummary } from '../api/client'
import { useParameterValues } from './parameterValues'

// Parts of a test case (scenario version, assets, parameters) and its
// client device, shown in the case execution list and on its own page.

/** Parameters as chips, `name=value unit`. */
export function Parameters({ testCase }: { testCase: TestCaseSummary }) {
  const { t } = useTranslation()
  const values = useParameterValues(testCase)
  if (values.length === 0) return t('common.none')

  return (
    <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5 }}>
      {values.map(({ name, label, uploaded }) => {
        const chip = (
          <Chip
            size="small"
            variant="outlined"
            label={t('runDetail.cases.parameter', { name, value: label })}
          />
        )
        return uploaded !== undefined ? (
          <Tooltip
            describeChild
            key={name}
            title={t('runDetail.cases.uploadedAs', { value: uploaded })}
          >
            {chip}
          </Tooltip>
        ) : (
          <Box component="span" key={name}>
            {chip}
          </Box>
        )
      })}
    </Box>
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
    <Tooltip describeChild title={t('runDetail.cases.uploadedAs', { value: uploaded })}>
      <Box
        component="span"
        sx={{ textDecoration: 'underline dotted', textUnderlineOffset: 3 }}
      >
        {shown}
      </Box>
    </Tooltip>
  )
}

/** The assets of a test case, one per line, the content hash in the tooltip. */
export function AssetList({ assets }: { assets: TestCaseSummary['assets'] }) {
  const { t } = useTranslation()
  if (assets.length === 0) return t('common.none')
  return assets.map((asset) => (
    <Tooltip describeChild key={asset.id} title={asset.content_hash ?? ''}>
      <Box component="span" sx={{ display: 'block', whiteSpace: 'nowrap' }}>
        {asset.version
          ? t('runDetail.cases.assetVersion', {
              name: asset.name,
              version: asset.version,
            })
          : asset.name}
      </Box>
    </Tooltip>
  ))
}

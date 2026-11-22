import Alert from '@mui/material/Alert'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { jsonTokens, prettyJson } from './jsonTokens'
import { CodeBlock } from './TextView'

/** Larger texts are shown without highlighting, to keep the page fast. */
const maxHighlightedChars = 250_000

/** Token colors from the theme, so they follow the color mode. */
const tokenSx = {
  '& [data-token="key"]': { color: 'primary.main' },
  '& [data-token="string"]': { color: 'success.main' },
  '& [data-token="number"]': { color: 'warning.main' },
  '& [data-token="boolean"]': { color: 'secondary.main' },
  '& [data-token="null"]': { color: 'text.secondary' },
  '& [data-token="other"]': { color: 'error.main' },
} as const

/**
 * A JSON attachment, pretty-printed and highlighted. Invalid JSON is shown
 * as uploaded, with a note.
 */
export function JsonView({ text }: { text: string }) {
  const { t } = useTranslation()
  const pretty = useMemo(() => prettyJson(text), [text])
  const shown = pretty ?? text
  const tokens = useMemo(
    () => (shown.length <= maxHighlightedChars ? jsonTokens(shown) : undefined),
    [shown],
  )
  return (
    <>
      {pretty === undefined && (
        <Alert severity="warning" sx={{ mb: 2 }}>
          {t('attachments.invalidJson')}
        </Alert>
      )}
      <CodeBlock sx={tokenSx}>
        {tokens
          ? tokens.map((token, index) =>
              token.type === 'whitespace' || token.type === 'punctuation' ? (
                token.text
              ) : (
                <span key={index} data-token={token.type}>
                  {token.text}
                </span>
              ),
            )
          : shown}
      </CodeBlock>
    </>
  )
}

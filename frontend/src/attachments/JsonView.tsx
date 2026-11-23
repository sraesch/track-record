import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { Alert } from '../ui'
import { jsonTokens, prettyJson } from './jsonTokens'
import { CodeBlock } from './TextView'
import './attachments.css'

/** Larger texts are shown without highlighting, to keep the page fast. */
const maxHighlightedChars = 250_000

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
        <Alert tone="warning" style={{ marginBottom: 'var(--space-4)' }}>
          {t('attachments.invalidJson')}
        </Alert>
      )}
      {/* Token colors from Graphite's tokens (attachments.css), so they
          follow the color mode. */}
      <CodeBlock className="tr-attachments-json">
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

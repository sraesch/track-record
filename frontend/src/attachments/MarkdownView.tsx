import { useTranslation } from 'react-i18next'
import Markdown, { type Components } from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { ExternalLink } from '../ui'
import './attachments.css'

/**
 * A Markdown attachment (GitHub flavor: tables, task lists, …), rendered
 * as React elements. HTML in the Markdown is dropped, never rendered
 * (ADR 0003), and unsafe link targets are removed by react-markdown.
 */
export function MarkdownView({ text }: { text: string }) {
  const { t } = useTranslation()
  const components: Components = {
    // Links leave the app in a new tab, without telling the target where
    // they came from.
    a: ({ href, children }) => (
      <ExternalLink href={href} target="_blank" rel="noopener noreferrer">
        {children}
      </ExternalLink>
    ),
    // Images are not loaded: a report must not make the browser fetch
    // from elsewhere. They are offered as links instead.
    img: ({ src, alt }) =>
      typeof src === 'string' && src ? (
        <ExternalLink href={src} target="_blank" rel="noopener noreferrer">
          {t('attachments.markdownImage', { alt: alt || src })}
        </ExternalLink>
      ) : null,
  }
  return (
    <div className="tr-attachments-markdown">
      <Markdown remarkPlugins={[remarkGfm]} skipHtml components={components}>
        {text}
      </Markdown>
    </div>
  )
}

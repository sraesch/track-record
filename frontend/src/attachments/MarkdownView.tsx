import Box from '@mui/material/Box'
import Link from '@mui/material/Link'
import { useTranslation } from 'react-i18next'
import Markdown, { type Components } from 'react-markdown'
import remarkGfm from 'remark-gfm'

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
      <Link href={href} target="_blank" rel="noopener noreferrer">
        {children}
      </Link>
    ),
    // Images are not loaded: a report must not make the browser fetch
    // from elsewhere. They are offered as links instead.
    img: ({ src, alt }) =>
      typeof src === 'string' && src ? (
        <Link href={src} target="_blank" rel="noopener noreferrer">
          {t('attachments.markdownImage', { alt: alt || src })}
        </Link>
      ) : null,
  }
  return (
    <Box sx={markdownSx}>
      <Markdown remarkPlugins={[remarkGfm]} skipHtml components={components}>
        {text}
      </Markdown>
    </Box>
  )
}

const markdownSx = {
  typography: 'body1',
  overflowWrap: 'anywhere',
  '& > :last-child': { mb: 0 },
  '& h1, & h2, & h3, & h4, & h5, & h6': {
    mt: 0,
    mb: 1.5,
    lineHeight: 1.3,
    fontWeight: 600,
  },
  // Space above a heading, except at the very top.
  '& * + h1, & * + h2, & * + h3, & * + h4, & * + h5, & * + h6': { mt: 3 },
  '& h1': { fontSize: '1.75rem' },
  '& h2': { fontSize: '1.4rem' },
  '& h3': { fontSize: '1.2rem' },
  '& h4, & h5, & h6': { fontSize: '1rem' },
  '& p, & ul, & ol, & blockquote, & pre, & table': { mt: 0, mb: 2 },
  '& ul, & ol': { pl: 3 },
  '& li + li': { mt: 0.5 },
  '& code': {
    fontFamily: 'monospace',
    fontSize: '0.875em',
    px: 0.5,
    borderRadius: 0.5,
    bgcolor: 'action.hover',
  },
  '& pre': {
    p: 2,
    borderRadius: 1,
    bgcolor: 'action.hover',
    overflowX: 'auto',
  },
  '& pre code': { p: 0, bgcolor: 'transparent' },
  '& blockquote': {
    mx: 0,
    pl: 2,
    borderLeft: 4,
    borderColor: 'divider',
    color: 'text.secondary',
  },
  '& table': { borderCollapse: 'collapse', display: 'block', overflowX: 'auto' },
  '& th, & td': { border: 1, borderColor: 'divider', px: 1.5, py: 0.75 },
  '& th': { fontWeight: 600, bgcolor: 'action.hover' },
  '& hr': { border: 0, borderTop: 1, borderColor: 'divider', my: 3 },
} as const

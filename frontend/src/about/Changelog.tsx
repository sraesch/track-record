import changelog from '../../APP_CHANGELOG.md?raw'
import { MarkdownView } from '../attachments/MarkdownView'

/**
 * What changed in each version, for the people who use the app
 * (APP_CHANGELOG.md, a shorter version of the repository's CHANGELOG.md).
 * The text is bundled with this module, so it is loaded with the Markdown
 * renderer, only when the About page is opened.
 */
export function Changelog() {
  return <MarkdownView text={changelog} />
}

import { Download, ExternalLink } from 'lucide-react'
import type { MouseEvent, ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router'
import type { AttachmentMeta, CaseExecutionSummary } from '../../api/client'
import { AttachmentThumbnail } from '../../attachments/AttachmentThumbnail'
import {
  attachmentContentUrl,
  attachmentKind,
  mediaType,
} from '../../attachments/kind'
import { formatBytes } from '../../format'
import { Section } from '../../pages/Section'
import { Time } from '../../pages/Time'
import {
  IconAnchorButton,
  IconLinkButton,
  Table,
  TextLink,
  VisuallyHidden,
} from '../../ui'
import { attachmentPath, caseExecutionPath } from '../paths'
import '../runs.css'

type Props = {
  attachments: readonly AttachmentMeta[]
  /** What a file is attached to: the run, a case execution or a sample. */
  attachedTo: (attachment: AttachmentMeta) => ReactNode
  /** The URL of this page with an attachment's preview dialog open. */
  previewHref: (attachmentId: string) => string
}

/**
 * Files uploaded with a run (FR-5): logs, screenshots, traces. Images,
 * Markdown, JSON and text open in a preview dialog or, in a new tab, on
 * their own page (images also show a thumbnail here); every file can be
 * downloaded.
 */
export function RunAttachments({
  attachments,
  attachedTo,
  previewHref,
}: Props) {
  const { t } = useTranslation()
  return (
    <Section title={t('runDetail.attachments.title')} flush>
      {attachments.length === 0 ? (
        <p className="tr-runs-note">{t('runDetail.attachments.empty')}</p>
      ) : (
        <Table aria-label={t('runDetail.attachments.title')}>
          <thead>
            <tr>
              <th>{t('runDetail.attachments.file')}</th>
              <th>{t('runDetail.attachments.attachedTo')}</th>
              <th>{t('runDetail.attachments.type')}</th>
              <th className="g-r">{t('runDetail.attachments.size')}</th>
              <th>{t('runDetail.attachments.uploaded')}</th>
              <th>
                <VisuallyHidden>
                  {t('runDetail.attachments.actions')}
                </VisuallyHidden>
              </th>
            </tr>
          </thead>
          <tbody>
            {attachments.map((attachment) => (
              <tr key={attachment.id}>
                {/* Names break between words, and only inside a word
                    longer than the column. */}
                <td className="tr-runs-file-cell">
                  <FileName
                    attachment={attachment}
                    previewHref={previewHref(attachment.id)}
                  />
                </td>
                <td>{attachedTo(attachment)}</td>
                <td>
                  <code className="ui-code">
                    {mediaType(attachment.content_type)}
                  </code>
                </td>
                <td className="g-r">{formatBytes(attachment.size_bytes)}</td>
                <td>
                  <Time iso={attachment.created_at} />
                </td>
                <td className="tr-runs-actions-cell">
                  {attachmentKind(attachment) && (
                    <IconLinkButton
                      size="sm"
                      icon={ExternalLink}
                      to={attachmentPath(attachment.run_id, attachment.id)}
                      target="_blank"
                      rel="noopener"
                      label={t('runDetail.attachments.openInNewTab', {
                        file: attachment.file_name,
                      })}
                      tooltip={t('attachments.openInNewTab')}
                    />
                  )}
                  <IconAnchorButton
                    size="sm"
                    icon={Download}
                    href={attachmentContentUrl(attachment.id)}
                    download={attachment.file_name}
                    label={t('runDetail.attachments.download', {
                      file: attachment.file_name,
                    })}
                    tooltip={t('attachments.download')}
                  />
                </td>
              </tr>
            ))}
          </tbody>
        </Table>
      )}
    </Section>
  )
}

/**
 * The file name, opening the preview where there is one (with a thumbnail
 * for images); otherwise plain text, the file is only downloaded. A plain
 * click opens the dialog; the link itself leads to the attachment's own
 * page, so opening it in a new tab (middle click, Ctrl/Cmd+click, the
 * context menu) shows the file on its own.
 */
function FileName({
  attachment,
  previewHref,
}: {
  attachment: AttachmentMeta
  previewHref: string
}) {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const kind = attachmentKind(attachment)
  if (!kind) return attachment.file_name
  return (
    <TextLink
      className="tr-runs-file-link"
      to={attachmentPath(attachment.run_id, attachment.id)}
      onClick={(event: MouseEvent) => {
        const modified =
          event.metaKey || event.ctrlKey || event.shiftKey || event.altKey
        if (event.button !== 0 || modified) return
        event.preventDefault()
        navigate(previewHref, { state: { preview: true } })
      }}
      aria-label={t('runDetail.attachments.open', {
        file: attachment.file_name,
      })}
    >
      {kind === 'image' && <AttachmentThumbnail attachment={attachment} />}
      {attachment.file_name}
    </TextLink>
  )
}

/** The run, a case execution or one of its samples. */
export function AttachedTo({
  attachment,
  caseExecutions,
}: {
  attachment: AttachmentMeta
  caseExecutions: ReadonlyMap<string, CaseExecutionSummary>
}) {
  const { t } = useTranslation()
  const id = attachment.case_execution_id
  if (!id) return t('runDetail.attachments.toRun')

  const ce = caseExecutions.get(id)
  const name = ce
    ? t('runDetail.attachments.caseName', {
        scenario: ce.test_case.scenario,
        device: ce.client_device.resolved_name,
      })
    : t('runDetail.attachments.caseUnknown')
  return (
    <TextLink to={caseExecutionPath(attachment.run_id, id)}>
      {attachment.sample_id
        ? t('runDetail.attachments.toSample', { name })
        : name}
    </TextLink>
  )
}

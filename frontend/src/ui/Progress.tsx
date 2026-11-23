// Indeterminate progress drawn with CSS: a styled <progress> cannot show the
// moving bar in every browser, so these are elements with role progressbar.
/* oxlint-disable jsx-a11y/prefer-tag-over-role */
import { cx } from '@graphite/react'
import { LoaderCircle } from 'lucide-react'
import type { CSSProperties } from 'react'
import { useTranslation } from 'react-i18next'

/** A thin bar for loading without a known end, e.g. above a table. */
export function ProgressBar({ label, className, style }: { label?: string; className?: string; style?: CSSProperties }) {
  const { t } = useTranslation()
  return (
    <div
      role="progressbar"
      aria-label={label ?? t('common.loading')}
      className={cx('ui-progress', className)}
      style={style}
    />
  )
}

/** A spinning circle, e.g. inside a field while it checks its value. */
export function Spinner({ size = 16, label }: { size?: number; label?: string }) {
  const { t } = useTranslation()
  return (
    <span role="progressbar" aria-label={label ?? t('common.loading')} style={{ display: 'inline-flex' }}>
      <LoaderCircle size={size} className="ui-spinner" aria-hidden />
    </span>
  )
}

type SkeletonProps = {
  /** `rect` (default) for blocks, `text` for a line of text. */
  variant?: 'rect' | 'text'
  width?: number | string
  height?: number | string
  /** Names it for screen readers (e.g. "Loading…"); otherwise it is hidden from them. */
  label?: string
  className?: string
}

/** Grey placeholder in the shape of content that is loading. */
export function Skeleton({ variant = 'rect', width, height, label, className }: SkeletonProps) {
  return (
    <span
      className={cx('ui-skeleton', variant === 'text' && 'ui-skeleton-text', className)}
      style={{ width, height }}
      {...(label ? { role: 'progressbar', 'aria-label': label } : { 'aria-hidden': true })}
    />
  )
}

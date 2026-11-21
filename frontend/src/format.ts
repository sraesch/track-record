import i18n from './i18n'

// Formatting of values for display, in the current UI language.

/** Date and time in the browser's time zone, e.g. "20 Nov 2026, 09:11". */
export function formatDateTime(iso: string): string {
  return new Intl.DateTimeFormat(i18n.language, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(iso))
}

/** Full timestamp with seconds and time zone, for tooltips. */
export function formatTimestamp(iso: string): string {
  return new Intl.DateTimeFormat(i18n.language, {
    dateStyle: 'full',
    timeStyle: 'long',
  }).format(new Date(iso))
}

/** Duration between two timestamps, e.g. "1 h 05 min", "4 min 10 s". */
export function formatDuration(startIso: string, endIso: string): string {
  const seconds = Math.max(
    0,
    Math.round((Date.parse(endIso) - Date.parse(startIso)) / 1000),
  )
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  const s = seconds % 60
  const pad = (n: number) => String(n).padStart(2, '0')
  if (h > 0) return i18n.t('duration.hours', { h, m: pad(m) })
  if (m > 0) return i18n.t('duration.minutes', { m, s: pad(s) })
  return i18n.t('duration.seconds', { s })
}

/** The first 8 characters of a commit hash. */
export function shortCommit(commit: string): string {
  return commit.slice(0, 8)
}

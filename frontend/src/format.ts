import i18n from './i18n'

// Formatting of values for display, in the current UI language.

// Building an Intl formatter is the expensive part; keep one per language
// and style instead of one per call (two per table row).
const dateTimeFormats = new Map<string, Intl.DateTimeFormat>()

function dateTimeFormat(
  style: 'short' | 'long',
  options: Intl.DateTimeFormatOptions,
) {
  const key = `${i18n.language}:${style}`
  let format = dateTimeFormats.get(key)
  if (!format) {
    format = new Intl.DateTimeFormat(i18n.language, options)
    dateTimeFormats.set(key, format)
  }
  return format
}

/** Date and time in the browser's time zone, e.g. "20 Nov 2026, 09:11". */
export function formatDateTime(iso: string): string {
  return dateTimeFormat('short', {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(iso))
}

/** Full timestamp with seconds and time zone, for tooltips. */
export function formatTimestamp(iso: string): string {
  return dateTimeFormat('long', {
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

const numberFormats = new Map<string, Intl.NumberFormat>()

function numberFormat(key: string, options: Intl.NumberFormatOptions) {
  const cacheKey = `${i18n.language}:${key}`
  let format = numberFormats.get(cacheKey)
  if (!format) {
    format = new Intl.NumberFormat(i18n.language, options)
    numberFormats.set(cacheKey, format)
  }
  return format
}

/**
 * A number as the user reads it. 15 significant digits keep every digit a
 * double can hold, without floating point noise (0.1 + 0.2 shows 0.3).
 */
export function formatNumber(value: number): string {
  return numberFormat('number', { maximumSignificantDigits: 15 }).format(value)
}

const byteUnits = ['byte', 'kilobyte', 'megabyte', 'gigabyte', 'terabyte']

/** A file size in decimal units, e.g. "512 byte", "1.2 MB". */
export function formatBytes(bytes: number): string {
  let unit = 0
  let value = bytes
  while (value >= 1000 && unit < byteUnits.length - 1) {
    value /= 1000
    unit += 1
  }
  const name = byteUnits[unit]!
  return numberFormat(name, {
    style: 'unit',
    unit: name,
    unitDisplay: name === 'byte' ? 'long' : 'short',
    maximumFractionDigits: 1,
  }).format(value)
}

/**
 * A JSON value (a parameter or an environment detail) for display. Numbers
 * are formatted, strings shown as they are, anything else as JSON.
 */
export function formatJsonValue(value: unknown): string {
  if (typeof value === 'number') return formatNumber(value)
  if (typeof value === 'string') return value
  return JSON.stringify(value)
}

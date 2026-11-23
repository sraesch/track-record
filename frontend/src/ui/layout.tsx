import { cx } from '@graphite/react'
import {
  type AllHTMLAttributes,
  type AnchorHTMLAttributes,
  type CSSProperties,
  type ElementType,
  type HTMLAttributes,
  type ReactNode,
  type TableHTMLAttributes,
} from 'react'
import { Link, type LinkProps } from 'react-router'
import { type Space, space } from './space'

type StackProps = Omit<AllHTMLAttributes<HTMLElement>, 'wrap'> & {
  /** Element to render. Default div. */
  as?: ElementType
  /** Lay the children out in a row instead of a column. */
  row?: boolean
  gap?: Space
  wrap?: boolean
  align?: CSSProperties['alignItems']
  justify?: CSSProperties['justifyContent']
  children?: ReactNode
}

/** Children in a column (or a row) with a gap between them. */
export function Stack({ as: As = 'div', row = false, gap = 0, wrap = false, align, justify, className, style, ...rest }: StackProps) {
  return (
    <As
      className={cx(row ? 'ui-row' : 'ui-stack', wrap && 'ui-wrap', className)}
      style={{ gap: space(gap), alignItems: align, justifyContent: justify, ...style }}
      {...rest}
    />
  )
}

/** A link inside the app (React Router), styled as a text link. */
export function TextLink({ className, ...rest }: LinkProps) {
  return <Link className={cx('ui-link', className)} {...rest} />
}

/** A link to a file or another site, styled as a text link. */
export function ExternalLink({ className, children, ...rest }: AnchorHTMLAttributes<HTMLAnchorElement>) {
  return (
    <a className={cx('ui-link', className)} {...rest}>
      {children}
    </a>
  )
}

type SurfaceProps = HTMLAttributes<HTMLElement> & {
  as?: ElementType
  /** Dashed border, for something that is not there yet. */
  dashed?: boolean
}

/** A bordered box on the panel background (Graphite's panel without a header). */
export function Surface({ as: As = 'div', dashed = false, className, ...rest }: SurfaceProps) {
  return <As className={cx('ui-surface', dashed && 'ui-surface-dashed', className)} {...rest} />
}

type TableProps = TableHTMLAttributes<HTMLTableElement> & {
  /** A border around the table; leave it out inside a Section. */
  framed?: boolean
  /** Row height: `compact` 28px (default), `default` 32px, `relaxed` 40px. */
  density?: 'compact' | 'default' | 'relaxed'
  /** Scroll vertically past this height; the header stays. */
  maxHeight?: number | string
  wrapClassName?: string
}

/**
 * Graphite's data table as plain markup: write thead/tbody/tr/th/td inside.
 * Cell classes: `g-r` right-aligns numbers, `g-muted` and `g-mono` for
 * secondary and code values, `ui-wrap-cell` lets long text wrap.
 */
export function Table({ framed = false, density = 'compact', maxHeight, wrapClassName, className, ...rest }: TableProps) {
  return (
    <div className={cx('ui-table-wrap', framed && 'ui-table-framed', wrapClassName)} style={maxHeight !== undefined ? { maxHeight } : undefined}>
      <table className={cx('g-table', density !== 'compact' && `g-table-${density}`, className)} {...rest} />
    </div>
  )
}

/** Hides content visually but keeps it for screen readers. */
export function VisuallyHidden({ as: As = 'span', ...rest }: HTMLAttributes<HTMLElement> & { as?: ElementType }) {
  return <As className="ui-sr-only" {...rest} />
}

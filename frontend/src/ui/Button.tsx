import { cx, type ButtonSize, type ButtonVariant } from '@graphite/react'
import { LoaderCircle, type LucideIcon } from 'lucide-react'
import { type AnchorHTMLAttributes, type ButtonHTMLAttributes, forwardRef, type ReactNode } from 'react'
import { Link, type LinkProps } from 'react-router'
import { Tooltip } from './Tooltip'

// Graphite's buttons (g-btn classes) with Lucide icons (ADR 0019): the
// library's own Button only takes its small bundled icon set.

type Look = {
  /** `primary` once per view; `secondary` (default); `ghost` for toolbars; `danger` for destructive actions. */
  variant?: ButtonVariant
  /** `sm` 24px, `md` 28px (default), `lg` 32px. */
  size?: ButtonSize
}

const classes = (variant: ButtonVariant, size: ButtonSize, iconOnly: boolean, className?: string) =>
  cx('g-btn', `g-btn-${variant}`, size !== 'md' && `g-btn-${size}`, iconOnly && 'g-btn-icon', className)

const iconSize = (size: ButtonSize, iconOnly: boolean) => (iconOnly && size !== 'sm' ? 16 : 14)

export type ButtonProps = Look &
  ButtonHTMLAttributes<HTMLButtonElement> & {
    /** Leading icon. */
    icon?: LucideIcon
    /** The action is under way: disabled, with a spinner in place of the icon. */
    loading?: boolean
    children: ReactNode
  }

/** Triggers an action. Verb-first, sentence-case labels. */
export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = 'secondary', size = 'md', icon: Icon, loading = false, disabled, className, children, type = 'button', ...rest },
  ref,
) {
  const s = iconSize(size, false)
  return (
    <button
      ref={ref}
      type={type}
      className={classes(variant, size, false, className)}
      disabled={disabled || loading}
      {...(loading ? { 'aria-busy': true } : {})}
      {...rest}
    >
      {loading ? <LoaderCircle size={s} className="ui-spinner" aria-hidden /> : Icon && <Icon size={s} aria-hidden />}
      {children}
    </button>
  )
})

export type IconButtonProps = Look &
  Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'children'> & {
    icon: LucideIcon
    /** Names the button; also shown as its tooltip. */
    label: string
    /** Tooltip text when it should say more than (or differ from) the label. */
    tooltip?: string
    loading?: boolean
  }

/** A button with only an icon: named by `label`, which its tooltip shows. */
export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  { variant = 'ghost', size = 'md', icon: Icon, label, tooltip, loading = false, disabled, className, type = 'button', ...rest },
  ref,
) {
  const s = iconSize(size, true)
  return (
    <Tooltip title={tooltip ?? label}>
      <button
        ref={ref}
        type={type}
        aria-label={label}
        className={classes(variant, size, true, className)}
        disabled={disabled || loading}
        {...(loading ? { 'aria-busy': true } : {})}
        {...rest}
      >
        {loading ? <LoaderCircle size={s} className="ui-spinner" aria-hidden /> : <Icon size={s} aria-hidden />}
      </button>
    </Tooltip>
  )
})

export type LinkButtonProps = Look &
  LinkProps & {
    icon?: LucideIcon
    children: ReactNode
  }

/** A link (client-side navigation) that looks like a button. */
export function LinkButton({ variant = 'secondary', size = 'md', icon: Icon, className, children, ...rest }: LinkButtonProps) {
  return (
    <Link className={classes(variant, size, false, className)} {...rest}>
      {Icon && <Icon size={iconSize(size, false)} aria-hidden />}
      {children}
    </Link>
  )
}

export type IconLinkButtonProps = Look &
  Omit<LinkProps, 'children'> & {
    icon: LucideIcon
    /** Names the link; also shown as its tooltip. */
    label: string
    /** Tooltip text when it should be shorter than (or differ from) the label. */
    tooltip?: string
  }

/** A link with only an icon, named by `label`, which its tooltip shows. */
export function IconLinkButton({ variant = 'ghost', size = 'md', icon: Icon, label, tooltip, className, ...rest }: IconLinkButtonProps) {
  return (
    <Tooltip title={tooltip ?? label}>
      <Link aria-label={label} className={classes(variant, size, true, className)} {...rest}>
        <Icon size={iconSize(size, true)} aria-hidden />
      </Link>
    </Tooltip>
  )
}

export type AnchorButtonProps = Look &
  AnchorHTMLAttributes<HTMLAnchorElement> & {
    icon?: LucideIcon
    children: ReactNode
  }

/** A plain link (a file to download, another site) that looks like a button. */
export function AnchorButton({ variant = 'secondary', size = 'md', icon: Icon, className, children, ...rest }: AnchorButtonProps) {
  return (
    <a className={classes(variant, size, false, className)} {...rest}>
      {Icon && <Icon size={iconSize(size, false)} aria-hidden />}
      {children}
    </a>
  )
}

export type IconAnchorButtonProps = Look &
  Omit<AnchorHTMLAttributes<HTMLAnchorElement>, 'children'> & {
    icon: LucideIcon
    /** Names the link; also shown as its tooltip. */
    label: string
    /** Tooltip text when it should be shorter than (or differ from) the label. */
    tooltip?: string
  }

/** A plain link with only an icon (e.g. a download), named by `label`. */
export function IconAnchorButton({ variant = 'ghost', size = 'md', icon: Icon, label, tooltip, className, ...rest }: IconAnchorButtonProps) {
  return (
    <Tooltip title={tooltip ?? label}>
      <a aria-label={label} className={classes(variant, size, true, className)} {...rest}>
        <Icon size={iconSize(size, true)} aria-hidden />
      </a>
    </Tooltip>
  )
}

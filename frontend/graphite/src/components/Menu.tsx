import { useCallback, useId, useMemo, useRef, useState, type KeyboardEvent, type ReactElement } from 'react';
import { cx } from '../utils/cx';
import { useDismiss } from '../utils/hooks';
import { Button, type ButtonSize, type ButtonVariant } from './Button';
import { Icon, type IconName } from './Icon';
import { Kbd } from './Kbd';

interface MenuEntryBase {
  id: string;
}

export interface MenuActionItem extends MenuEntryBase {
  type: 'item';
  label: string;
  icon?: IconName;
  shortcut?: string;
  disabled?: boolean;
  /** Destructive: shown in danger colour; place last, after a separator. */
  danger?: boolean;
  onSelect: () => void;
}

export interface MenuCheckboxItem extends MenuEntryBase {
  type: 'checkbox';
  label: string;
  icon?: IconName;
  shortcut?: string;
  disabled?: boolean;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
}

export interface MenuSeparator extends MenuEntryBase {
  type: 'separator';
}

export interface MenuGroupLabel extends MenuEntryBase {
  type: 'group';
  label: string;
}

export type MenuEntry = MenuActionItem | MenuCheckboxItem | MenuSeparator | MenuGroupLabel;
type Actionable = MenuActionItem | MenuCheckboxItem;

const isActionable = (e: MenuEntry): e is Actionable => (e.type === 'item' || e.type === 'checkbox') && e.disabled !== true;

export interface MenuListProps {
  items: readonly MenuEntry[];
  /** Called after an item is chosen (e.g. to close a popover). */
  onClose?: () => void;
  'aria-label'?: string;
  className?: string;
  /** Focus the list on mount so arrow keys work immediately. */
  autoFocus?: boolean;
}

/** The menu surface. Use directly for context menus; `Menu` wraps it with a trigger. */
export function MenuList({ items, onClose, className, autoFocus = false, ...aria }: MenuListProps): ReactElement {
  const actionable = items.filter(isActionable);
  const [active, setActive] = useState(0);
  const activeId = actionable[active]?.id;

  const run = (entry: Actionable): void => {
    if (entry.type === 'item') entry.onSelect();
    else entry.onCheckedChange(!entry.checked);
    onClose?.();
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>): void => {
    const n = actionable.length;
    if (n === 0) return;
    if (e.key === 'ArrowDown') { e.preventDefault(); setActive((a) => (a + 1) % n); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); setActive((a) => (a - 1 + n) % n); }
    else if (e.key === 'Home') { e.preventDefault(); setActive(0); }
    else if (e.key === 'End') { e.preventDefault(); setActive(n - 1); }
    else if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      const entry = actionable[active];
      if (entry !== undefined) run(entry);
    } else if (e.key === 'Escape') { e.preventDefault(); onClose?.(); }
  };

  return (
    <div
      role="menu"
      tabIndex={-1}
      className={cx('g-menu', className)}
      onKeyDown={onKeyDown}
      ref={(el) => { if (autoFocus && el !== null && !el.contains(document.activeElement)) el.focus(); }}
      {...(aria['aria-label'] !== undefined ? { 'aria-label': aria['aria-label'] } : {})}
    >
      {items.map((entry) => {
        switch (entry.type) {
          case 'separator':
            return <div key={entry.id} role="separator" className="g-menu-sep" />;
          case 'group':
            return <div key={entry.id} role="presentation" className="g-menu-group">{entry.label}</div>;
          case 'item':
          case 'checkbox': {
            const isCheck = entry.type === 'checkbox';
            return (
              <div
                key={entry.id}
                role={isCheck ? 'menuitemcheckbox' : 'menuitem'}
                aria-disabled={entry.disabled === true}
                {...(isCheck ? { 'aria-checked': entry.checked } : {})}
                className={cx('g-menu-item', entry.type === 'item' && entry.danger === true && 'g-menu-danger')}
                {...(entry.id === activeId ? { 'data-active': '' } : {})}
                onPointerEnter={() => { const i = actionable.indexOf(entry); if (i >= 0) setActive(i); }}
                onClick={() => { if (entry.disabled !== true) run(entry); }}
              >
                {entry.icon !== undefined ? <Icon name={entry.icon} size="sm" /> : isCheck && <Icon name="check" size="sm" style={{ visibility: entry.checked ? 'visible' : 'hidden' }} />}
                {entry.label}
                {entry.shortcut !== undefined && <Kbd>{entry.shortcut}</Kbd>}
              </div>
            );
          }
        }
      })}
    </div>
  );
}

interface MenuTriggerCommon {
  items: readonly MenuEntry[];
  variant?: ButtonVariant;
  size?: ButtonSize;
  /** Align the menu to the trigger's start (default) or end edge. */
  align?: 'start' | 'end';
  className?: string;
}

export type MenuProps =
  | (MenuTriggerCommon & { label: string; icon?: IconName; iconOnly?: false })
  | (MenuTriggerCommon & { label: string; icon: IconName; iconOnly: true });

/** A button that opens a menu: overflow (⋯) menus, column menus, view options. */
export function Menu({ items, label, icon, iconOnly = false, variant = 'ghost', size = 'md', align = 'start', className }: MenuProps): ReactElement {
  const [open, setOpen] = useState(false);
  const wrap = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const refs = useMemo(() => [wrap], []);
  const close = useCallback(() => { setOpen(false); trigger.current?.focus(); }, []);
  useDismiss(refs, open, useCallback(() => setOpen(false), []));
  const menuId = useId();
  const common = {
    ref: trigger,
    variant,
    size,
    'aria-haspopup': 'menu' as const,
    'aria-expanded': open,
    'aria-controls': menuId,
    onClick: () => setOpen((o) => !o),
    onKeyDown: (e: KeyboardEvent<HTMLButtonElement>) => { if (e.key === 'ArrowDown') { e.preventDefault(); setOpen(true); } },
  };
  return (
    <div ref={wrap} className={cx('g-anchor', className)}>
      {iconOnly && icon !== undefined ? (
        <Button {...common} icon={icon} iconOnly aria-label={label} />
      ) : (
        <Button {...common} {...(icon !== undefined ? { icon } : {})}>{label}</Button>
      )}
      {open && (
        <div id={menuId} className={cx('g-pop', align === 'end' && 'g-pop-end')}>
          <MenuList items={items} onClose={close} aria-label={label} autoFocus />
        </div>
      )}
    </div>
  );
}

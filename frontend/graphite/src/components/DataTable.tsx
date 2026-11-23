import { useMemo, type CSSProperties, type ReactElement, type ReactNode } from 'react';
import { cx } from '../utils/cx';
import { useControllableState } from '../utils/hooks';
import { Checkbox } from './Checkbox';
import { Icon } from './Icon';

export type SortDirection = 'ascending' | 'descending';

export interface SortState {
  columnId: string;
  direction: SortDirection;
}

export interface Column<Row> {
  /** Stable id, used for sort state. */
  id: string;
  /** Header text. Put units here, not in cells. */
  header: string;
  /** Renders the cell. Return pre-formatted values. */
  cell: (row: Row) => ReactNode;
  /** Right-align numbers. Default `left`. */
  align?: 'left' | 'right';
  /** `muted` for secondary dimensions, `mono` for IDs and field names. */
  variant?: 'default' | 'muted' | 'mono';
  /** Makes the column sortable; returns the raw value to compare. */
  sortValue?: (row: Row) => string | number;
  width?: number | string;
}

export interface DataTableProps<Row> {
  columns: readonly Column<Row>[];
  rows: readonly Row[];
  getRowId: (row: Row) => string;
  /** Accessible name of the table. */
  'aria-label': string;
  /** Row height: `compact` 28px (default), `default` 32px, `relaxed` 40px. */
  density?: 'compact' | 'default' | 'relaxed';
  sort?: SortState | null;
  defaultSort?: SortState | null;
  onSortChange?: (sort: SortState | null) => void;
  /** Sort rows in the component (default). Set false when the server sorts. */
  sortRows?: boolean;
  /** Show a checkbox column. */
  selectable?: boolean;
  selectedIds?: ReadonlySet<string>;
  defaultSelectedIds?: ReadonlySet<string>;
  onSelectionChange?: (ids: ReadonlySet<string>) => void;
  onRowClick?: (row: Row) => void;
  /** Totals row content, one cell per column (selection column excluded). */
  footer?: readonly ReactNode[];
  /** Scroll vertically past this height; the header stays sticky. */
  maxHeight?: number | string;
  emptyMessage?: ReactNode;
  className?: string;
}

const EMPTY: ReadonlySet<string> = new Set<string>();

function compare(a: string | number, b: string | number): number {
  if (typeof a === 'number' && typeof b === 'number') return a - b;
  return String(a).localeCompare(String(b), undefined, { numeric: true });
}

/** Dense, sortable, selectable rows. */
export function DataTable<Row>({
  columns,
  rows,
  getRowId,
  density = 'compact',
  sort,
  defaultSort = null,
  onSortChange,
  sortRows = true,
  selectable = false,
  selectedIds,
  defaultSelectedIds = EMPTY,
  onSelectionChange,
  onRowClick,
  footer,
  maxHeight,
  emptyMessage = 'No rows match the current filters.',
  className,
  ...aria
}: DataTableProps<Row>): ReactElement {
  const [currentSort, setSort] = useControllableState<SortState | null>(sort, defaultSort, onSortChange);
  const [selected, setSelected] = useControllableState<ReadonlySet<string>>(selectedIds, defaultSelectedIds, onSelectionChange);

  const sorted = useMemo(() => {
    if (!sortRows || currentSort === null) return rows;
    const col = columns.find((c) => c.id === currentSort.columnId);
    const key = col?.sortValue;
    if (key === undefined) return rows;
    const dir = currentSort.direction === 'ascending' ? 1 : -1;
    return [...rows].sort((a, b) => compare(key(a), key(b)) * dir);
  }, [rows, columns, currentSort, sortRows]);

  const toggleSort = (col: Column<Row>): void => {
    if (currentSort?.columnId !== col.id) setSort({ columnId: col.id, direction: col.align === 'right' ? 'descending' : 'ascending' });
    else if (currentSort.direction === 'descending') setSort({ columnId: col.id, direction: 'ascending' });
    else setSort({ columnId: col.id, direction: 'descending' });
  };

  const ids = sorted.map(getRowId);
  const selectedCount = ids.filter((id) => selected.has(id)).length;
  const toggleAll = (): void => setSelected(selectedCount === ids.length ? new Set<string>() : new Set<string>(ids));
  const toggleRow = (id: string): void => {
    const next = new Set(selected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setSelected(next);
  };

  const wrapStyle: CSSProperties | undefined = maxHeight !== undefined ? { maxHeight } : undefined;
  const cellClass = (c: Column<Row>): string => cx(c.align === 'right' && 'g-r', c.variant === 'muted' && 'g-muted', c.variant === 'mono' && 'g-mono');

  return (
    <div className={cx('g-table-wrap', className)} style={wrapStyle}>
      <table className={cx('g-table', density !== 'compact' && `g-table-${density}`)} aria-label={aria['aria-label']}>
        <thead>
          <tr>
            {selectable && (
              <th className="g-w-check">
                <Checkbox
                  aria-label="Select all rows"
                  checked={ids.length > 0 && selectedCount === ids.length}
                  indeterminate={selectedCount > 0 && selectedCount < ids.length}
                  onCheckedChange={toggleAll}
                />
              </th>
            )}
            {columns.map((col) => {
              const sortable = col.sortValue !== undefined;
              const dir = currentSort?.columnId === col.id ? currentSort.direction : undefined;
              return (
                <th
                  key={col.id}
                  className={cx(col.align === 'right' && 'g-r')}
                  style={col.width !== undefined ? { width: col.width } : undefined}
                  {...(sortable ? { 'aria-sort': dir ?? 'none' } : {})}
                >
                  {sortable ? (
                    <button type="button" className="g-th-sort" onClick={() => toggleSort(col)}>
                      {col.header}
                      {dir !== undefined && <Icon name={dir === 'ascending' ? 'arrow-up' : 'arrow-down'} size="sm" />}
                    </button>
                  ) : (
                    col.header
                  )}
                </th>
              );
            })}
          </tr>
        </thead>
        <tbody>
          {sorted.length === 0 ? (
            <tr>
              <td className="g-table-empty" colSpan={columns.length + (selectable ? 1 : 0)}>{emptyMessage}</td>
            </tr>
          ) : (
            sorted.map((row) => {
              const id = getRowId(row);
              const isSelected = selected.has(id);
              return (
                <tr
                  key={id}
                  aria-selected={selectable ? isSelected : undefined}
                  {...(onRowClick !== undefined ? { 'data-clickable': '', onClick: () => onRowClick(row) } : {})}
                >
                  {selectable && (
                    <td className="g-w-check" onClick={(e) => e.stopPropagation()}>
                      <Checkbox aria-label={`Select row ${id}`} checked={isSelected} onCheckedChange={() => toggleRow(id)} />
                    </td>
                  )}
                  {columns.map((col) => (
                    <td key={col.id} className={cellClass(col)}>{col.cell(row)}</td>
                  ))}
                </tr>
              );
            })
          )}
        </tbody>
        {footer !== undefined && (
          <tfoot>
            <tr>
              {selectable && <td />}
              {columns.map((col, i) => (
                <td key={col.id} className={cellClass(col)}>{footer[i]}</td>
              ))}
            </tr>
          </tfoot>
        )}
      </table>
    </div>
  );
}

export interface TableBarProps {
  value: number;
  /** Column maximum; the bar is 64px at max. */
  max: number;
}

/** In-cell bar for one measure per table, placed before the formatted value. */
export function TableBar({ value, max }: TableBarProps): ReactElement {
  const w = max > 0 ? Math.max(2, Math.round((Math.max(0, value) / max) * 64)) : 0;
  return <span className="g-bar-cell" style={{ width: w }} aria-hidden="true" />;
}

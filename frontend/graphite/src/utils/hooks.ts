import { useCallback, useEffect, useRef, useState, type MutableRefObject, type Ref, type RefObject } from 'react';

/**
 * State that is controlled when `value` is defined and uncontrolled otherwise.
 * `onChange` fires in both modes.
 */
export function useControllableState<T>(
  value: T | undefined,
  defaultValue: T,
  onChange: ((next: T) => void) | undefined,
): [T, (next: T) => void] {
  const [inner, setInner] = useState<T>(defaultValue);
  const controlled = value !== undefined;
  const current = controlled ? value : inner;
  const set = useCallback(
    (next: T) => {
      if (!controlled) setInner(next);
      onChange?.(next);
    },
    [controlled, onChange],
  );
  return [current, set];
}

/** Calls `handler` on a pointer-down outside every element in `refs` while `active`. */
export function useDismiss(
  refs: readonly RefObject<HTMLElement | null>[],
  active: boolean,
  handler: () => void,
): void {
  useEffect(() => {
    if (!active) return undefined;
    const onDown = (event: PointerEvent): void => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (refs.some((r) => r.current?.contains(target) === true)) return;
      handler();
    };
    document.addEventListener('pointerdown', onDown);
    return () => document.removeEventListener('pointerdown', onDown);
  }, [refs, active, handler]);
}

/** Merge several refs into one callback ref. */
export function useMergedRef<T>(...refs: readonly (Ref<T> | undefined)[]): (node: T | null) => void {
  const latest = useRef(refs);
  latest.current = refs;
  return useCallback((node: T | null) => {
    for (const ref of latest.current) {
      if (typeof ref === 'function') ref(node);
      else if (ref !== null && ref !== undefined) (ref as MutableRefObject<T | null>).current = node;
    }
  }, []);
}

/** Width of an element, tracked with ResizeObserver. */
export function useElementWidth<T extends HTMLElement>(fallback: number): [RefObject<T | null>, number] {
  const ref = useRef<T>(null);
  const [width, setWidth] = useState(fallback);
  useEffect(() => {
    const el = ref.current;
    if (el === null) return undefined;
    setWidth(el.clientWidth || fallback);
    if (typeof ResizeObserver === 'undefined') return undefined;
    const ro = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (entry !== undefined) setWidth(Math.max(1, Math.round(entry.contentRect.width)));
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [fallback]);
  return [ref, width];
}

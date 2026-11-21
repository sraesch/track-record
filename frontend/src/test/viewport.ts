/**
 * Emulates a screen width for `window.matchMedia` in jsdom. Understands
 * `(min-width:Npx)` and `(max-width:Npx)`, as MUI's breakpoints produce;
 * every other query (e.g. prefers-color-scheme) does not match.
 */
export function setViewportWidth(width: number) {
  window.matchMedia = (query: string): MediaQueryList => {
    const conditions = [...query.matchAll(/\((min|max)-width:\s*([\d.]+)px\)/g)]
    const matches =
      conditions.length > 0 &&
      conditions.every(([, kind, px]) =>
        kind === 'min' ? width >= Number(px) : width <= Number(px),
      )
    return {
      matches,
      media: query,
      onchange: null,
      addListener: () => {},
      removeListener: () => {},
      addEventListener: () => {},
      removeEventListener: () => {},
      dispatchEvent: () => false,
    }
  }
}

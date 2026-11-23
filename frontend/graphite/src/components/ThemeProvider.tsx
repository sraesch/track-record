import { createContext, useContext, useEffect, useMemo, useState, type ReactElement, type ReactNode } from 'react';

export type ThemeName = 'dark' | 'light';
export type ThemePreference = ThemeName | 'system';

interface ThemeContextValue {
  preference: ThemePreference;
  resolved: ThemeName;
}

const ThemeContext = createContext<ThemeContextValue>({ preference: 'dark', resolved: 'dark' });

export interface ThemeProviderProps {
  /** `system` follows the OS `prefers-color-scheme`. Default `dark`. */
  theme?: ThemePreference;
  /** Element that receives `data-theme`. Defaults to `<html>`. */
  target?: HTMLElement;
  children?: ReactNode;
}

function systemTheme(): ThemeName {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return 'dark';
  return window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
}

/** Sets `data-theme` on the document root so every Graphite token resolves for that theme. */
export function ThemeProvider({ theme = 'dark', target, children }: ThemeProviderProps): ReactElement {
  const [system, setSystem] = useState<ThemeName>(systemTheme);

  useEffect(() => {
    if (theme !== 'system' || typeof window.matchMedia !== 'function') return undefined;
    const mq = window.matchMedia('(prefers-color-scheme: light)');
    const onChange = (): void => setSystem(mq.matches ? 'light' : 'dark');
    onChange();
    mq.addEventListener('change', onChange);
    return () => mq.removeEventListener('change', onChange);
  }, [theme]);

  const resolved: ThemeName = theme === 'system' ? system : theme;

  useEffect(() => {
    const el = target ?? document.documentElement;
    el.setAttribute('data-theme', resolved);
    el.style.colorScheme = resolved;
  }, [resolved, target]);

  const value = useMemo(() => ({ preference: theme, resolved }), [theme, resolved]);
  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

/** The active theme preference and the theme it resolves to. */
export function useTheme(): ThemeContextValue {
  return useContext(ThemeContext);
}

# @graphite/react

React components for **Graphite**, a minimal dark-first design system for dense, data-heavy desktop apps (BI, data exploration, analysis tools) built on web technology: Electron, Tauri or the browser.

- React 18+, written in strict TypeScript (`strict`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, …).
- Dark and Light themes from one token set; switch with `ThemeProvider`.
- No runtime dependencies besides React. Styling is plain CSS over design tokens.

## Install and build

```bash
npm install
npm run typecheck   # src, examples and previews under the strict config
npm run build       # dist/: index.js (ESM), index.d.ts, graphite.css, tokens.css
```

To use it in an app before publishing to a registry: `npm pack`, then `npm install ./graphite-react-0.1.0.tgz`, or add it to your monorepo workspace.

## Use

```tsx
import '@graphite/react/tokens.css';
import '@graphite/react/styles.css';
import { ThemeProvider, AppShell, Breadcrumbs, NavGroup, NavItem, Panel, LineChart } from '@graphite/react';

export function App() {
  return (
    <ThemeProvider theme="system">
      <div style={{ height: '100vh' }}>
        <AppShell
          title={<Breadcrumbs items={['Graphite', 'Sales', 'Revenue overview']} />}
          sidebar={<NavGroup label="Workspace"><NavItem icon="grid" label="Dashboards" current href="#" /></NavGroup>}
        >
          <div style={{ padding: 16 }}>
            <Panel title="Weekly revenue" subtitle="€, thousands">
              <LineChart aria-label="Weekly revenue" xLabels={['W44', 'W45', 'W46']}
                series={[{ id: 'online', label: 'Online', slot: 1, values: [92, 98, 104] }]} />
            </Panel>
          </div>
        </AppShell>
      </div>
    </ThemeProvider>
  );
}
```

`examples/Dashboard.tsx` is a complete dashboard; `examples/type-tests.tsx` shows the mistakes the types reject.

## Components

| Area | Exports |
| --- | --- |
| Layout | `AppShell`, `Breadcrumbs`, `NavGroup`, `NavItem`, `Panel` |
| Data | `DataTable`, `TableBar`, `LineChart`, `BarList`, `ChartLegend`, `chartColor`, `StatTile`, `Sparkline`, `Badge` |
| Inputs | `Button`, `TextField`, `Select`, `Checkbox`, `Switch`, `RadioGroup`, `Tabs`, `SegmentedControl` |
| Filters & overlays | `FilterBar`, `FilterChip`, `FilterBarSeparator`, `Menu`, `MenuList`, `Tooltip`, `TooltipContent` |
| Foundations | `ThemeProvider`, `useTheme`, `Icon`, `Kbd`, `cx`, `useControllableState` |

Typing conventions:

- Every stateful component is **controlled or uncontrolled**: `value` + `onChange`, or `defaultValue`.
- `Select`, `RadioGroup`, `Tabs` and `SegmentedControl` are generic over a string union; `DataTable` is generic over the row type.
- Unions close the vocabulary: `IconName`, `BadgeTone`, `ButtonVariant`, `ChartSlot`, `MenuEntry`.
- Accessibility is in the types: icon-only buttons and segments require `aria-label`; tables and charts require one too.

## Layout of this package

```
src/            components and hooks
styles/         tokens.css (generated from the design system's tokens.json) and graphite.css
examples/       Dashboard.tsx, type-tests.tsx
previews/       sources of the design-system preview cards
scripts/        build.mjs (package), build-ds.mjs and build-previews.mjs (design-system bundle and cards)
```

The design system (tokens, guidelines, live previews) is the source of truth for values. When tokens change there, copy its `project/tokens.css` and `project/components/bundle.css` into `styles/` (as `tokens.css` and `graphite.css`).

## Icons

The bundled icon set is a small Lucide-style subset drawn for Graphite. For a full set, use `lucide-react` at 16px with `strokeWidth={1.5}`. It inherits `currentColor` the same way.

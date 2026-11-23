# ADR 0015: Charting library

**Status:** proposed · **Date:** 2026-11-22 · colors from Graphite's tokens
since [ADR 0019](0019-graphite-design-system.md)

## Context

The web UI needs charts: series of a sample on the case execution page
first, then series overlays with percentile bands (FR-19) and trends with
markers for annotations, device detail changes and environment changes
(FR-18). The library was left open in
[ADR 0010](0010-frontend-scaffold.md) and [ADR 0011](0011-ui-shell.md).

What the charts have to handle:

- **Volume:** up to ~10k points per series (NFR-3). The backend downsamples
  for display (`max_points`, keeping each bucket's minimum and maximum), so
  a chart gets about one to two thousand points per line, but several
  lines: one per sample, and later one per sample of several runs.
- **Each line has its own x values.** Samples are not on a common grid;
  aligning them is an explicit operation (FR-19), not a precondition for
  drawing them.
- **Bands and marks:** filled areas between two lines (percentile bands),
  vertical marks and labelled points on a time axis (FR-18), zoom into a
  part of a long series.
- **Light and dark** color schemes from the MUI theme
  ([ADR 0011](0011-ui-shell.md)). The theme uses CSS variables, which a
  canvas cannot read.
- **Accessibility:** a description for screen readers; the numbers stay
  available as text.
- TypeScript strict, React 19, MUI 9, Vite; maintained.

Candidates, measured as a minified and gzipped ESM bundle of a line chart
with tooltip, legend and reference lines (zoom where available), with
React and MUI as externals (esbuild, November 2026):

| | Rendering | Bundle (gzip) | Own x per line | Bands, marks, zoom | Theming | Accessibility | Maintenance |
|---|---|---|---|---|---|---|---|
| **Apache ECharts 6** (tree-shaken `echarts/core`) | canvas or SVG | ~213 kB | yes (`[x, y]` pairs) | built in: stacked areas, `markLine`/`markPoint`/`markArea`, `dataZoom` | colors in the option | `aria` (description, decal patterns) | Apache project, regular releases |
| **Recharts 3** | SVG, React | ~139 kB | yes (`data` per `<Line>`) | `Area`, `ReferenceLine`, `Brush` | props | `accessibilityLayer` (keyboard) | active |
| **MUI X Charts 9** (Community) | SVG, React | ~118 kB | no: series share the x axis data | reference lines; zoom and pan only in Pro (paid) | MUI theme, CSS variables | keyboard navigation | MUI, active |
| **uPlot 1.6** | canvas | ~24 kB | no: one shared x array (nulls to align) | bands built in; marks, tooltip and legend by hand (hooks) | colors per instance | none | one maintainer, last release 03/2025 |
| visx / Nivo | SVG (visx: primitives) | — | visx: everything by hand | visx: by hand | props | by hand | active / slow |

- **MUI X Charts** fits the theme best, but needs every line of a chart on
  the same x values: overlaying samples would mean merging their x values
  and filling gaps. Zoom is in the paid tier.
- **Recharts** renders every point through React and SVG; with several lines
  of a thousand points or more, hovering and re-rendering get slow, and its
  bundle is not much smaller than ECharts'.
- **uPlot** is by far the smallest and fastest, but tooltips, legends, marks
  and accessibility would all be our own code, and it also wants one shared
  x array.
- **visx** is a toolkit for building a chart library, more code than we want
  to own; **Nivo** has neither per-line x values with good performance nor
  the marks we need.

## Decision

- **Apache ECharts 6**, imported through `echarts/core` with only the
  charts, components and the canvas renderer we use (`src/charts/echarts.ts`),
  so unused chart types stay out of the bundle.
- **Our own thin wrapper** (`src/charts/EChart.tsx`), not
  `echarts-for-react`: it creates the chart on a `<div>`, applies the option
  on change, follows the container size (`ResizeObserver`) and disposes the
  chart on unmount. That is a few dozen lines, and we control updates and the
  ECharts version.
- **Loaded lazily** as its own chunk (`charts` in `vite.config.ts`), like
  the Markdown renderer ([ADR 0013](0013-attachment-previews.md)): only
  pages with a chart load it, and it stays cached across app releases.
- **Colors from the MUI theme:** a hook (`useChartColors`) resolves the
  palette of the active color scheme (`useColorScheme`, including the system
  preference) to concrete colors — text, divider, background and a line
  palette — and options are built from them. Switching the scheme rebuilds
  the option. ECharts themes are not used.
- **Canvas renderer.** Many long lines draw faster on a canvas than as SVG.
- **Accessibility:** every chart has a `role="img"` container with a
  translated description (what is plotted, how many lines, the axes), and
  the numbers stay available as text next to it (e.g. the series table on
  the case execution page). Lines differ by color and, where it carries
  meaning (warmup), by dash pattern.
- **Option building is plain code:** pages turn their data into an ECharts
  option in a pure function, which is unit-tested; component tests replace
  the canvas chart (jsdom has no canvas).
- **Downsampling stays on the server** (`max_points`); the client draws what
  it gets.

## Consequences

- New dependency: `echarts` (with `zrender`). The chart chunk (line chart,
  grid, legend, tooltip, canvas) is about 175 kB gzipped, loaded only on
  pages with charts; each further component adds to it.
- Chart texts (axis names, legend, tooltips) come from our translations;
  ECharts' own built-in texts are not shown.
- Trends (FR-18) use `markLine`/`markPoint` for annotations and changes, and
  `dataZoom` for long time ranges; overlays with bands (FR-19) use stacked
  areas. Both without further dependencies.

## When to revisit

The chart chunk becomes a problem for load times, or charts with many more
lines (e.g. overlays across hundreds of runs) are too slow on a canvas
(then: uPlot or WebGL rendering for those views).

## Unreleased

The first version of Track Record: a place that keeps your benchmark
results and helps you analyze them.

### Runs

- Browse all runs, newest first, and filter them by suite, status, branch,
  device, tags, date, SUT version, scenario, metric, parameters and more.
  Invalid runs are hidden unless you ask for them.
- Open a run to see its details, tags, annotations, test cases and
  attachments.
- Mark a run as invalid with a reason, or valid again. Delete a run for
  good by typing its ID.
- Preview attachments right in the app: images, Markdown, JSON and text.
  Every file can also be downloaded or opened in its own tab.

### Test case results

- See the measurements of every sample, with warmup samples dimmed.
- Every series is drawn as a chart, with one line per sample.
- Derived metrics are calculated for each result, and the app shows when
  one has no value.
- Tag and annotate a test case; every result of that test case shows them.

### Definitions

- **Suites & scenarios:** see every version of a suite or scenario and what
  changed between them, so you know why a trend starts over.
- **Metrics:** edit, rename and delete metric definitions, and browse the
  catalogue of units.
- **Derived metrics:** write formulas over your metrics in an editor that
  checks them as you type, and try them on a result before saving. Derived
  metrics can be changed, renamed and deleted.
- **Aliases:** give devices and parameter values another name, so that
  results recorded under different names are treated as one.

### Administration

- Create API tokens for your benchmark harness and other tools, and revoke
  them. A token's secret is shown only once.

### Everywhere

- Every page has its own link, filters included, so you can share exactly
  what you see.
- A new, denser look made for working with lots of data, in a dark and a
  light theme.
- Light and dark mode, following your system or set by you.
- The side menu can be collapsed to a rail of icons. On narrow screens it
  slides in over the page.
- This page: open **About** in the app bar to see what's new.

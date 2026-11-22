import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { formatDateTime } from '../../format'
import { mockApi, problem } from '../../test/api'
import { renderApp } from '../../test/render'
import {
  caseExecutionDetail,
  caseExecutionPath,
  json,
  runDetail,
  runId,
  runPath,
  sample,
} from '../detail/fixtures'

const pagePath = (id = 'ce1', run = runId) => `/runs/${run}/cases/${id}`

const section = (name: string) => screen.findByRole('region', { name })
const tableRows = async (name: string) =>
  within(await screen.findByRole('table', { name }))
    .getAllByRole('row')
    .map((row) =>
      within(row)
        .getAllByRole(row.querySelector('th') ? 'columnheader' : 'cell')
        .map((cell) => cell.textContent),
    )

describe('Case execution', () => {
  it('shows what the case execution is', async () => {
    mockApi({
      [runPath]: () => runDetail(),
      [caseExecutionPath('ce1')]: () => caseExecutionDetail('ce1'),
    })
    renderApp(pagePath())

    expect(
      await screen.findByRole('heading', {
        level: 1,
        name: 'orbit · pixel-8-lab-1',
      }),
    ).toBeInTheDocument()
    expect(document.title).toBe('orbit · pixel-8-lab-1 · Track Record')
    expect(screen.getByRole('link', { name: 'Back to the run' })).toHaveAttribute(
      'href',
      `/runs/${runId}`,
    )

    const overview = within(await section('Overview'))
    expect(
      await overview.findByRole('link', {
        name: `streaming-nightly · ${formatDateTime('2026-11-20T03:05:00Z')}`,
      }),
    ).toHaveAttribute('href', `/runs/${runId}`)
    expect(overview.getByText('Completed')).toBeInTheDocument()
    expect(overview.getByText('orbit')).toBeInTheDocument()
    expect(overview.getByText('v2')).toBeInTheDocument()
    expect(overview.getByText('(1 warmup)')).toBeInTheDocument()
    expect(overview.getByText('(1 min 30 s)')).toBeInTheDocument()
    expect(overview.getByText('city-block 3')).toBeInTheDocument()
    expect(overview.getByText('ce1')).toBeInTheDocument()

    const parameters = within(await section('Parameters'))
    expect(parameters.getByText('bandwidth')).toBeInTheDocument()
    expect(parameters.getByText('20 Mbit/s')).toBeInTheDocument()
    expect(parameters.getByText('draco')).toBeInTheDocument()

    const device = within(await section('Client device details'))
    expect(device.getByText('os.version')).toBeInTheDocument()
    expect(device.getByText('Pixel 8')).toBeInTheDocument()
  })

  it('shows names and values after aliases, the uploaded ones on hover', async () => {
    const user = userEvent.setup()
    const ce = caseExecutionDetail('ce1')
    mockApi({
      [runPath]: () => runDetail(),
      [caseExecutionPath('ce1')]: () => ({
        ...ce,
        test_case: {
          ...ce.test_case,
          parameters: json({ bandwidth: 20, pipeline: 'meshopt-v1' }),
          resolved_parameters: json({ bandwidth: 20, pipeline: 'meshopt' }),
        },
        client_device: { id: 2, name: 'pixel8-lab1', resolved_name: 'pixel-8-lab-1' },
      }),
    })
    renderApp(pagePath())

    await user.hover(await screen.findByText('meshopt'))
    expect(await screen.findByRole('tooltip')).toHaveTextContent(
      'Uploaded as meshopt-v1',
    )
    await user.unhover(screen.getByText('meshopt'))
    await user.hover(
      within(await section('Overview')).getByText('pixel-8-lab-1'),
    )
    expect(await screen.findByRole('tooltip')).toHaveTextContent(
      'Uploaded as pixel8-lab1',
    )
  })

  it('lists the measurements of every sample in repetition order', async () => {
    const samples = [
      sample(2),
      sample(0),
      // A metric not measured in every sample.
      sample(1, {
        measurements: [
          ...sample(1).measurements,
          {
            metric: 'peak_memory',
            metric_version: 1,
            metric_version_id: 7,
            unit: 'MiB',
            value: 463.3,
          },
        ],
      }),
    ]
    mockApi({
      [runPath]: () => runDetail(),
      [caseExecutionPath('ce1')]: () =>
        caseExecutionDetail('ce1', { samples }),
    })
    renderApp(pagePath())

    expect(await tableRows('Measurements')).toEqual([
      ['Repetition', 'frames_droppedcount', 'load_times', 'peak_memoryMiB'],
      ['0Warmup', '8', '5.9', '—'],
      ['1', '7', '6', '463.3'],
      ['2', '6', '6.1', '—'],
    ])
    expect(
      within(await section('Measurements')).getByText('3 samples, 1 warmup'),
    ).toBeInTheDocument()
  })

  it('names the version where a metric has several', async () => {
    const v2 = sample(1, {
      measurements: [
        {
          metric: 'load_time',
          metric_version: 2,
          metric_version_id: 9,
          unit: 's',
          value: 4.2,
        },
      ],
    })
    mockApi({
      [runPath]: () => runDetail(),
      [caseExecutionPath('ce1')]: () =>
        caseExecutionDetail('ce1', { samples: [sample(0), v2] }),
    })
    renderApp(pagePath())

    expect((await tableRows('Measurements'))[0]).toEqual([
      'Repetition',
      'frames_droppedcount',
      'load_time v1s',
      'load_time v2s',
    ])
  })

  it('lists the series of every sample', async () => {
    mockApi({
      [runPath]: () => runDetail(),
      [caseExecutionPath('ce1')]: () => caseExecutionDetail('ce1'),
    })
    renderApp(pagePath())

    expect(await tableRows('Series')).toEqual([
      ['Repetition', 'Metric', 'Unit', 'x', 'Points'],
      ['0Warmup', 'fps', 'fps', 'time (s)', '161'],
      ['1', 'fps', 'fps', 'time (s)', '161'],
      ['2', 'fps', 'fps', 'time (s)', '161'],
    ])
    expect(within(await section('Series')).getByText('3 series')).toBeInTheDocument()
  })

  it('plots the series of every sample, a line per sample', async () => {
    const points = (id: string) => () => ({
      id,
      x: [0, 0.5, 1, 1.5],
      y: [60, 58, 59, 61],
    })
    const api = mockApi({
      [runPath]: () => runDetail(),
      [caseExecutionPath('ce1')]: () => caseExecutionDetail('ce1'),
      '/ui/api/v1/series/fps-s0/points': points('fps-s0'),
      '/ui/api/v1/series/fps-s1/points': points('fps-s1'),
      '/ui/api/v1/series/fps-s2/points': points('fps-s2'),
    })
    renderApp(pagePath())

    const series = within(await section('Series'))
    const chart = within(await series.findByTestId('chart'))
    expect(
      series.getByText('fps over time (s): 3 lines, one per sample'),
    ).toBeInTheDocument()
    expect(chart.getAllByRole('listitem').map((li) => li.textContent)).toEqual([
      'Repetition 0 (warmup): 4 points',
      'Repetition 1: 4 points',
      'Repetition 2: 4 points',
    ])
    // Downsampled on the server for display (FR-19).
    expect(
      api.requests('/ui/api/v1/series/fps-s1/points')[0]?.searchParams.get(
        'max_points',
      ),
    ).toBe('1000')
  })

  it('shows an error with a retry when series points fail to load', async () => {
    const user = userEvent.setup()
    let fail = true
    const points = (id: string) => () =>
      fail && id === 'fps-s1'
        ? problem(500, 'internal', 'database down')
        : { id, x: [0, 1], y: [60, 61] }
    mockApi({
      [runPath]: () => runDetail(),
      [caseExecutionPath('ce1')]: () => caseExecutionDetail('ce1'),
      '/ui/api/v1/series/fps-s0/points': points('fps-s0'),
      '/ui/api/v1/series/fps-s1/points': points('fps-s1'),
      '/ui/api/v1/series/fps-s2/points': points('fps-s2'),
    })
    renderApp(pagePath())

    const series = within(await section('Series'))
    expect(
      await series.findByText(
        'The series could not be loaded: database down',
      ),
    ).toBeInTheDocument()
    fail = false
    await user.click(series.getByRole('button', { name: 'Retry' }))
    expect(
      within(await series.findByTestId('chart')).getAllByRole('listitem'),
    ).toHaveLength(3)
  })

  it('says when there are no measurements, series or parameters', async () => {
    const ce = caseExecutionDetail('ce1', {
      samples: [sample(0, { measurements: [], series: [] })],
    })
    mockApi({
      [runPath]: () => runDetail(),
      [caseExecutionPath('ce1')]: () => ({
        ...ce,
        test_case: {
          ...ce.test_case,
          parameters: json({}),
          resolved_parameters: json({}),
        },
        device_details: json({}),
      }),
    })
    renderApp(pagePath())

    expect(await screen.findByText('No measurements uploaded.')).toBeInTheDocument()
    expect(screen.getByText('No series uploaded.')).toBeInTheDocument()
    expect(screen.getByText('This test case has no parameters.')).toBeInTheDocument()
    expect(screen.getByText('No device details uploaded.')).toBeInTheDocument()
  })

  it('lists only the files attached to this case execution and its samples', async () => {
    const attachment = runDetail().attachments[1]!
    mockApi({
      [runPath]: () =>
        runDetail({
          attachments: [
            ...runDetail().attachments,
            { ...attachment, id: 'f3', sample_id: null, file_name: 'trace.json', content_type: 'application/json' },
            { ...attachment, id: 'f4', case_execution_id: 'ce9', file_name: 'other.png' },
          ],
        }),
      [caseExecutionPath('ce2')]: () => caseExecutionDetail('ce2'),
    })
    renderApp(pagePath('ce2'))

    const attachments = within(await section('Attachments'))
    expect(await attachments.findByText('frame.png')).toBeInTheDocument()
    expect(attachments.getByText('Repetition 1')).toBeInTheDocument()
    expect(attachments.getByText('trace.json')).toBeInTheDocument()
    expect(attachments.getByText('Case execution')).toBeInTheDocument()
    expect(attachments.queryByText('harness.log')).not.toBeInTheDocument()
    expect(attachments.queryByText('other.png')).not.toBeInTheDocument()
  })

  it('opens an attachment in the preview through the URL', async () => {
    const user = userEvent.setup()
    mockApi({
      [runPath]: () => runDetail(),
      [caseExecutionPath('ce2')]: () => caseExecutionDetail('ce2'),
      '/ui/api/v1/attachments/f2/content': () =>
        new Response(new Blob(['png']), {
          headers: { 'content-type': 'image/png' },
        }),
    })
    const router = renderApp(pagePath('ce2'))

    await user.click(
      await within(await section('Attachments')).findByRole('link', {
        name: 'Open frame.png',
      }),
    )
    expect(await screen.findByRole('dialog')).toBeInTheDocument()
    expect(router.state.location.search).toBe('?attachment=f2')
  })

  it('links a superseded case execution to its correction and back', async () => {
    const user = userEvent.setup()
    mockApi({
      [runPath]: () => runDetail(),
      [caseExecutionPath('ce1')]: () =>
        caseExecutionDetail('ce1', { superseded_by: 'ce4' }),
      [caseExecutionPath('ce4')]: () =>
        caseExecutionDetail('ce4', { supersedes_id: 'ce1' }),
    })
    const router = renderApp(pagePath('ce1'))

    expect(await screen.findByText('Replaced by a correction')).toBeInTheDocument()
    await user.click(screen.getByRole('link', { name: 'Open the correction' }))
    expect(router.state.location.pathname).toBe(pagePath('ce4'))
    expect(await screen.findByText('Correction')).toBeInTheDocument()
    expect(
      screen.getByRole('link', { name: 'Open the replaced upload' }),
    ).toHaveAttribute('href', pagePath('ce1'))
  })

  it('warns when the run is invalid', async () => {
    mockApi({
      [runPath]: () =>
        runDetail({ is_valid: false, invalid_reason: 'Wrong backend' }),
      [caseExecutionPath('ce1')]: () => caseExecutionDetail('ce1'),
    })
    renderApp(pagePath())

    expect(
      await screen.findByText('Marked invalid: Wrong backend'),
    ).toBeInTheDocument()
    expect(
      screen.getByText(/analysis leaves this case execution out/),
    ).toBeInTheDocument()
  })

  it('leads to the URL under its own run', async () => {
    mockApi({
      [runPath]: () => runDetail(),
      [caseExecutionPath('ce1')]: () => caseExecutionDetail('ce1'),
    })
    const otherRun = '11111111-2222-4333-8444-555555555555'
    const router = renderApp(`${pagePath('ce1', otherRun)}?attachment=f2`)

    expect(
      await screen.findByRole('heading', { level: 1, name: 'orbit · pixel-8-lab-1' }),
    ).toBeInTheDocument()
    expect(router.state.location.pathname).toBe(pagePath('ce1'))
    expect(router.state.location.search).toBe('?attachment=f2')
  })

  it('says when the case execution does not exist', async () => {
    mockApi({
      [runPath]: () => runDetail(),
      [caseExecutionPath('ce1')]: () =>
        problem(404, 'not_found', 'no such case execution'),
    })
    renderApp(pagePath())

    expect(
      await screen.findByText(/There is no case execution ce1/),
    ).toBeInTheDocument()
  })

  it('shows an error with a retry when loading fails', async () => {
    const user = userEvent.setup()
    let fail = true
    mockApi({
      [runPath]: () => runDetail(),
      [caseExecutionPath('ce1')]: () =>
        fail
          ? problem(500, 'internal', 'database down')
          : caseExecutionDetail('ce1'),
    })
    renderApp(pagePath())

    expect(
      await screen.findByText(/The case execution could not be loaded/),
    ).toBeInTheDocument()
    fail = false
    await user.click(screen.getByRole('button', { name: 'Retry' }))
    expect(
      await screen.findByRole('heading', { level: 1, name: 'orbit · pixel-8-lab-1' }),
    ).toBeInTheDocument()
  })
})

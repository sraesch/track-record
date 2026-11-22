import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type { TestCaseSummary, VersionedDefinition } from '../api/client'
import { mockApi } from '../test/api'
import { enterText } from '../test/input'
import { currentQuery, renderApp } from '../test/render'

const suitesPath = '/ui/api/v1/suites'
const scenariosPath = '/ui/api/v1/scenarios'
const testCasesPath = '/ui/api/v1/test-cases'

// Uploaded JSON objects are plain objects in the generated types.
const json = (value: object) => value as Record<string, never>

const definition = (
  id: number,
  name: string,
  ...definitions: object[]
): VersionedDefinition => ({
  id,
  name,
  versions: definitions.map((d, index) => ({
    id: id * 10 + index,
    version: index + 1,
    definition: json(d),
    created_at: `2026-11-0${index + 1}T08:00:00Z`,
  })),
})

const suites = () => [
  definition(1, 'streaming-nightly', { samples: 3 }, { samples: 5, warmup: 1 }),
  definition(2, 'smoke', { samples: 1 }),
]

const scenarios = () => [
  definition(
    1,
    'orbit',
    { camera: { path: 'orbit' }, cache: 'cold' },
    { camera: { path: 'orbit-v2' }, duration_s: 40 },
  ),
  definition(2, 'startup', {}),
]

const testCase = (
  id: number,
  version: number,
  fields: Partial<TestCaseSummary> = {},
): TestCaseSummary => ({
  id,
  scenario: 'orbit',
  scenario_version: version,
  scenario_version_id: version,
  parameters: json({ bandwidth: 20 }),
  resolved_parameters: json({ bandwidth: 20 }),
  parameter_units: json({ bandwidth: 'Mbit/s' }),
  assets: [{ id: 1, name: 'city-block', version: '3', content_hash: 'abc' }],
  tags: [],
  ...fields,
})

const testCases = () => [
  testCase(11, 1),
  testCase(21, 2, { tags: ['heavy'] }),
  testCase(22, 2, {
    parameters: json({ bandwidth: 100 }),
    resolved_parameters: json({ bandwidth: 100 }),
  }),
]

const panel = (name: string) => screen.findByRole('region', { name })
const row = (table: HTMLElement, name: RegExp) =>
  within(within(table).getByRole('row', { name }))

describe('Suites & scenarios', () => {
  it('lists the suites and shows the versions of the selected one', async () => {
    mockApi({ [suitesPath]: suites })
    renderApp('/suites?name=streaming-nightly')

    const list = within(await screen.findByRole('navigation', { name: 'Suites' }))
    expect(await list.findByRole('link', { name: /^smoke/ })).toHaveAttribute(
      'href',
      '/suites?name=smoke',
    )
    expect(
      list.getByRole('link', { name: /^streaming-nightly/ }),
    ).toHaveAttribute('aria-current', 'page')
    expect(list.getByText('2 versions')).toBeInTheDocument()
    expect(list.getByText('2 suites')).toBeInTheDocument()

    const detail = within(await panel('streaming-nightly'))
    expect(
      detail.getByRole('link', { name: 'Runs of this suite' }),
    ).toHaveAttribute('href', '/runs?suite=streaming-nightly')
    const versions = detail.getByRole('table', { name: 'Versions' })
    // The latest version is shown; a suite has no test cases.
    const latest = row(versions, /^v2/)
    expect(latest.getByRole('link', { name: 'v2' })).toHaveAttribute(
      'aria-current',
      'true',
    )
    expect(latest.getByText('Changed: samples')).toBeInTheDocument()
    expect(latest.getByText('Added: warmup')).toBeInTheDocument()
    expect(row(versions, /^v1/).getByText('First version')).toBeInTheDocument()
    expect(
      within(versions).queryByRole('columnheader', { name: 'Test cases' }),
    ).toBeNull()
    expect(screen.queryByRole('region', { name: /^Test cases/ })).toBeNull()

    const current = within(await panel('Definition of version 2'))
    expect(current.getByText('Version 1: 3')).toBeInTheDocument()
    expect(current.getByText('New in this version')).toBeInTheDocument()
  })

  it('shows a scenario version with its changes and test cases', async () => {
    const api = mockApi({
      [scenariosPath]: scenarios,
      [testCasesPath]: testCases,
    })
    const router = renderApp('/suites?tab=scenarios&name=orbit')

    const detail = within(await panel('orbit'))
    expect(
      detail.getByRole('link', { name: 'Runs with this scenario' }),
    ).toHaveAttribute('href', '/runs?scenario=orbit')
    const versions = detail.getByRole('table', { name: 'Versions' })
    const latest = row(versions, /^v2/)
    expect(latest.getByText('Changed: camera.path')).toBeInTheDocument()
    expect(latest.getByText('Removed: cache')).toBeInTheDocument()
    expect(await latest.findByRole('cell', { name: '2' })).toBeInTheDocument()
    expect(api.requests(testCasesPath)[0]?.searchParams.get('scenario')).toBe(
      'orbit',
    )

    const current = within(await panel('Definition of version 2'))
    expect(current.getByText('Version 1: orbit')).toBeInTheDocument()
    expect(current.getByText('Removed in this version')).toBeInTheDocument()

    const cases = within(await panel('Test cases of version 2'))
    expect(cases.getByText('2 test cases')).toBeInTheDocument()
    expect(cases.getByText('bandwidth=100 Mbit/s')).toBeInTheDocument()
    expect(cases.getAllByText('city-block 3')).toHaveLength(2)
    expect(cases.getByRole('link', { name: 'Test case 21' })).toHaveAttribute(
      'href',
      '/suites?tab=scenarios&name=orbit&version=2&case=21',
    )
    expect(cases.queryByRole('link', { name: 'Test case 11' })).toBeNull()

    // An older version has its own definition and test cases.
    const user = userEvent.setup()
    await user.click(row(versions, /^v1/).getByRole('link', { name: 'v1' }))
    expect(currentQuery(router)).toBe('tab=scenarios&name=orbit&version=1')
    const first = within(await panel('Test cases of version 1'))
    expect(first.getByRole('link', { name: 'Test case 11' })).toBeInTheDocument()
    expect(
      within(await panel('Definition of version 1')).queryByText(/Version \d:/),
    ).toBeNull()
  })

  it('tags a test case and shows the test cases with a tag', async () => {
    let cases = testCases()
    const tagged: string[] = []
    mockApi({
      [scenariosPath]: scenarios,
      [testCasesPath]: () => cases,
      '/ui/api/v1/annotations': (url) => [
        {
          id: 'a1',
          test_case_id: Number(url.searchParams.get('test_case_id')),
          body: 'Needs the large cache',
          created_at: '2026-11-20T09:00:00Z',
          updated_at: '2026-11-20T09:00:00Z',
        },
      ],
      [`${testCasesPath}/22/tags/flaky`]: (_, request) => {
        tagged.push(request.method)
        cases = cases.map((tc) =>
          tc.id === 22 ? { ...tc, tags: ['flaky'] } : tc,
        )
        return new Response(null, { status: 204 })
      },
    })
    const router = renderApp('/suites?tab=scenarios&name=orbit&version=2&case=22')

    expect(
      await within(await panel('Annotations of test case 22')).findByText(
        'Needs the large cache',
      ),
    ).toBeInTheDocument()
    const tags = within(await panel('Tags of test case 22'))
    const user = userEvent.setup()
    await enterText(user, tags.getByRole('textbox', { name: 'New tag' }), 'flaky')
    await user.click(tags.getByRole('button', { name: 'Add tag' }))

    // The test cases are loaded again; tag and table show the new tag.
    const show = await tags.findByRole('button', {
      name: 'Show the test cases tagged flaky',
    })
    expect(tagged).toEqual(['PUT'])
    const table = within(await panel('Test cases of version 2'))
    expect(
      table.getByRole('link', { name: 'Show the test cases tagged heavy' }),
    ).toHaveAttribute(
      'href',
      '/suites?tab=scenarios&name=orbit&version=2&case=22&tag=heavy',
    )

    await user.click(show)
    expect(currentQuery(router)).toBe(
      'tab=scenarios&name=orbit&version=2&case=22&tag=flaky',
    )
    expect(await table.findByText('1 of 2 tagged flaky')).toBeInTheDocument()
    expect(table.queryByRole('link', { name: 'Test case 21' })).toBeNull()
    expect(table.getByRole('link', { name: 'Test case 22' })).toBeInTheDocument()
  })

  it('searches the list and keeps the search across the tabs', async () => {
    mockApi({ [suitesPath]: suites, [scenariosPath]: scenarios })
    const router = renderApp('/suites?name=smoke')

    const user = userEvent.setup()
    await enterText(
      user,
      await screen.findByRole('searchbox', { name: 'Search suites' }),
      'st',
    )
    const list = within(screen.getByRole('navigation', { name: 'Suites' }))
    await waitFor(() =>
      expect(list.queryByRole('link', { name: /^smoke/ })).toBeNull(),
    )
    expect(list.getByText('1 suite')).toBeInTheDocument()
    expect(currentQuery(router)).toBe('q=st&name=smoke')

    // The selected suite is not a scenario.
    await user.click(screen.getByRole('tab', { name: 'Scenarios' }))
    expect(currentQuery(router)).toBe('tab=scenarios&q=st')
    const scenarioList = within(
      await screen.findByRole('navigation', { name: 'Scenarios' }),
    )
    expect(
      await scenarioList.findByRole('link', { name: /^startup/ }),
    ).toBeInTheDocument()
    expect(scenarioList.queryByRole('link', { name: /^orbit/ })).toBeNull()
    expect(
      screen.getByText('Select a scenario to see its versions.'),
    ).toBeInTheDocument()
  })

  it('says what does not exist', async () => {
    mockApi({ [scenariosPath]: scenarios, [testCasesPath]: testCases })
    const router = renderApp('/suites?tab=scenarios&name=nope')
    expect(
      await screen.findByText('There is no scenario nope.'),
    ).toBeInTheDocument()

    await router.navigate('/suites?tab=scenarios&name=orbit&version=9')
    expect(
      await screen.findByText('orbit has no version 9. Select one of its versions.'),
    ).toBeInTheDocument()
    expect(screen.queryByRole('region', { name: /^Test cases/ })).toBeNull()

    // Test case 11 belongs to version 1.
    await router.navigate('/suites?tab=scenarios&name=orbit&version=2&case=11')
    expect(
      await screen.findByText('Version 2 has no test case 11.'),
    ).toBeInTheDocument()
  })

  it('says when nothing was uploaded yet', async () => {
    mockApi()
    renderApp('/suites')
    expect(
      await screen.findByText(
        'No suites yet. The harness uploads them with its results.',
      ),
    ).toBeInTheDocument()
    expect(screen.getByText('No suites yet.')).toBeInTheDocument()
  })
})

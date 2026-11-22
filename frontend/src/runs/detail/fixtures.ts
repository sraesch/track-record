// Test data for the run detail page: a run with all its parts, as the UI
// API returns it. Tests change what they need through `overrides`.
import type { CaseExecutionSummary, RunDetail } from '../../api/client'

export const runId = '0b7e3f52-1c3d-4f7a-9a51-1e2d3c4b5a69'
export const runPath = `/ui/api/v1/runs/${runId}`
export const casesPath = `${runPath}/case-executions`

// Uploaded JSON objects are plain objects in the generated types.
export const json = (value: object) => value as Record<string, never>

export function runDetail(overrides: Partial<RunDetail> = {}): RunDetail {
  return {
    id: runId,
    suite: { suite_version_id: 1, name: 'streaming-nightly', version: 2 },
    sut_version: {
      id: 1,
      commit: '168fed94a5013f65ea34157e7163f37114d6752d',
      branch: 'main',
      build_config: 'release',
      tag: 'v1.5.0',
    },
    shared_environment_id: 3,
    shared_environment: json({
      backend: { name: 'hub', version: '2.4.0' },
      region: 'eu-central',
    }),
    triggered_by: 'nightly',
    status: 'completed',
    started_at: '2026-11-20T03:05:00Z',
    ended_at: '2026-11-20T03:44:00Z',
    is_valid: true,
    invalid_reason: null,
    created_at: '2026-11-20T03:05:02Z',
    last_upload_at: '2026-11-20T03:44:10Z',
    case_execution_count: 3,
    tags: ['nightly', 'release'],
    annotations: [
      {
        id: 'a1',
        run_id: runId,
        body: 'Switched to the new compression',
        created_at: '2026-11-20T09:00:00Z',
        updated_at: '2026-11-20T09:00:00Z',
      },
    ],
    attachments: [
      {
        id: 'f1',
        run_id: runId,
        file_name: 'harness.log',
        content_type: 'text/plain',
        size_bytes: 1536,
        sha256: 'ab',
        created_at: '2026-11-20T03:44:00Z',
      },
      {
        id: 'f2',
        run_id: runId,
        case_execution_id: 'ce2',
        sample_id: 's1',
        file_name: 'frame.png',
        content_type: 'image/png',
        size_bytes: 2_400_000,
        sha256: 'cd',
        created_at: '2026-11-20T03:20:00Z',
      },
    ],
    ...overrides,
  }
}

export function caseExecution(
  id: string,
  overrides: {
    scenario?: string
    device?: string
    parameters?: object
    superseded_by?: string
    supersedes_id?: string
  } = {},
): CaseExecutionSummary {
  const parameters = overrides.parameters ?? { bandwidth: 20, pipeline: 'draco' }
  const device = overrides.device ?? 'pixel-8-lab-1'
  return {
    id,
    run_id: runId,
    test_case: {
      id: 1,
      scenario: overrides.scenario ?? 'orbit',
      scenario_version: 2,
      scenario_version_id: 5,
      parameters: json(parameters),
      resolved_parameters: json(parameters),
      parameter_units: json({ bandwidth: 'Mbit/s' }),
      assets: [{ id: 1, name: 'city-block', version: '3', content_hash: 'abc' }],
      tags: [],
    },
    client_device: { id: 1, name: device, resolved_name: device },
    device_environment_id: 7,
    created_at: '2026-11-20T03:10:00Z',
    started_at: '2026-11-20T03:06:00Z',
    ended_at: '2026-11-20T03:07:30Z',
    sample_count: 4,
    superseded_by: overrides.superseded_by ?? null,
    supersedes_id: overrides.supersedes_id ?? null,
  }
}

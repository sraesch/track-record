// URLs of a run and its parts (FR-21).

export const runPath = (runId: string) => `/runs/${runId}`

export const caseExecutionPath = (runId: string, caseExecutionId: string) =>
  `/runs/${runId}/cases/${caseExecutionId}`

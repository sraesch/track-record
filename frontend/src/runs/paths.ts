// URLs of a run and its parts (FR-21).

export const runPath = (runId: string) => `/runs/${runId}`

export const caseExecutionPath = (runId: string, caseExecutionId: string) =>
  `/runs/${runId}/cases/${caseExecutionId}`

/** An attachment on its own page (the preview dialog is `?attachment=`). */
export const attachmentPath = (runId: string, attachmentId: string) =>
  `/runs/${runId}/attachments/${attachmentId}`

// Seeds a Track Record server with realistic test data for UI development.
//
//   npm run seed                          local stack (deploy/local/forward.sh)
//   npm run seed -- --ui http://localhost:8081 --api http://localhost:8080
//                                         a server started with `cargo run`
//
// Everything goes through the public APIs, as a harness and a user would:
// ingestion on the machine port with a temporary `write` token (created and
// revoked through the UI port), everything else on the UI port.
//
// The data is deterministic (fixed dates, seeded random numbers, derived
// UUIDs). The script refuses to run twice against the same database; start
// from an empty one (deploy/local/down.sh and up.sh) to seed again.
//
// What it creates, ending on 2026-11-20 (see describe() below for details):
// - 30 nightly runs of `streaming-nightly`: 3 lab devices x 2 models x
//   2 bandwidths x 2 pipelines, 1 warmup + 3 samples each, with `loaded` and
//   `fps` series. History: a meshopt decode regression (days 12-17), a
//   backend upgrade (day 15), a device OS update (day 20), better draco
//   compression (day 22, v1.5.0 on day 21), a new scenario version (day 24).
// - CI `smoke` runs on feature branches, without series.
// - A failed run with partial data, an invalidated run, a corrected case
//   execution, an experiment run, and a run left open (aborted after the
//   stale timeout, 1 h by default).
// - Aliases (a renamed device, a renamed parameter value), derived metrics
//   (one with no value for incomplete loads), tags, annotations, attachments.
import { createHash } from 'node:crypto'
import { parseArgs } from 'node:util'

const { values: args } = parseArgs({
  options: {
    ui: { type: 'string', default: 'http://localhost:8088' },
    api: { type: 'string', default: 'http://localhost:8089' },
    concurrency: { type: 'string', default: '6' },
    help: { type: 'boolean', short: 'h', default: false },
  },
})

if (args.help) {
  console.log(`Usage: npm run seed -- [--ui URL] [--api URL] [--concurrency N]

  --ui           UI port base URL (default http://localhost:8088)
  --api          machine port base URL (default http://localhost:8089)
  --concurrency  parallel uploads (default 6)`)
  process.exit(0)
}

const UI = `${args.ui.replace(/\/$/, '')}/ui/api/v1`
const API = `${args.api.replace(/\/$/, '')}/api/v1`
const CONCURRENCY = Math.max(1, Number(args.concurrency))

// ---------------------------------------------------------------------------
// Deterministic helpers

/** A UUID (version 5 layout) derived from a key, so reruns use the same IDs. */
function uuid(key: string): string {
  const h = createHash('sha1').update(`track-record-seed:${key}`).digest()
  h[6] = (h[6]! & 0x0f) | 0x50
  h[8] = (h[8]! & 0x3f) | 0x80
  const x = h.subarray(0, 16).toString('hex')
  return `${x.slice(0, 8)}-${x.slice(8, 12)}-${x.slice(12, 16)}-${x.slice(16, 20)}-${x.slice(20)}`
}

function sha(algorithm: 'sha1' | 'sha256', text: string): string {
  return createHash(algorithm).update(text).digest('hex')
}

interface Random {
  next(): number
  normal(): number
}

/** A seeded random number generator (mulberry32), one per key. */
function random(key: string): Random {
  let a = createHash('sha1').update(key).digest().readUInt32LE(0)
  const next = () => {
    a = (a + 0x6d2b79f5) | 0
    let t = Math.imul(a ^ (a >>> 15), 1 | a)
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
  const normal = () =>
    Math.sqrt(-2 * Math.log(1 - next())) * Math.cos(2 * Math.PI * next())
  return { next, normal }
}

const round = (value: number, digits: number) =>
  Math.round(value * 10 ** digits) / 10 ** digits

// ---------------------------------------------------------------------------
// HTTP

type Json = null | boolean | number | string | Json[] | { [key: string]: Json }

async function call(
  method: string,
  url: string,
  body?: Json | Uint8Array,
  headers: Record<string, string> = {},
): Promise<Json> {
  const binary = body instanceof Uint8Array
  const init: RequestInit = {
    method,
    headers: {
      ...(body !== undefined && !binary ? { 'content-type': 'application/json' } : {}),
      ...headers,
    },
    body: body === undefined ? undefined : binary ? body : JSON.stringify(body),
  }
  // PUTs and GETs are idempotent; retry them on connection errors (a
  // port-forward may drop). Never retry POSTs.
  const attempts = method === 'PUT' || method === 'GET' ? 4 : 1
  for (let attempt = 1; ; attempt++) {
    let res: Response
    try {
      res = await fetch(url, init)
    } catch (err) {
      if (attempt >= attempts) throw new Error(`${method} ${url}: ${String(err)}`)
      await new Promise((r) => setTimeout(r, 500 * attempt))
      continue
    }
    const text = await res.text()
    if (!res.ok) throw new Error(`${method} ${url}: ${res.status} ${text}`)
    return text ? (JSON.parse(text) as Json) : null
  }
}

/** Runs tasks with at most `limit` in flight. */
async function pool<T>(limit: number, tasks: Array<() => Promise<T>>): Promise<T[]> {
  const results: T[] = new Array(tasks.length)
  let nextIndex = 0
  const worker = async () => {
    while (nextIndex < tasks.length) {
      const i = nextIndex++
      results[i] = await tasks[i]!()
    }
  }
  await Promise.all(Array.from({ length: Math.min(limit, tasks.length) }, worker))
  return results
}

// ---------------------------------------------------------------------------
// Time line: DAYS days ending on LAST_DAY. Day 0 is the oldest.

const DAYS = 30
const DAY_MS = 86_400_000
const LAST_DAY = Date.UTC(2026, 10, 20)

function at(day: number, hours = 0, minutes = 0, seconds = 0): Date {
  return new Date(
    LAST_DAY - (DAYS - 1 - day) * DAY_MS + ((hours * 60 + minutes) * 60 + seconds) * 1000,
  )
}

const iso = (t: Date) => t.toISOString()
const isWeekend = (day: number) => [0, 6].includes(at(day).getUTCDay())

// Release tags are set on the commit of these days.
const RELEASES: Record<number, string> = { 0: 'v1.4.0', 21: 'v1.5.0' }

/** The main commit a nightly run of `day` builds: no commits on weekends. */
function mainSut(day: number) {
  let commitDay = day
  while (commitDay > 0 && isWeekend(commitDay)) commitDay--
  return {
    commit: sha('sha1', `main-${commitDay}`),
    branch: 'main',
    tag: RELEASES[commitDay] ?? null,
    build_config: 'release',
  }
}

// ---------------------------------------------------------------------------
// The benchmark world

interface Asset {
  name: string
  version: string
  mb: number
}

const ASSETS: Asset[] = [
  { name: 'engine-block.glb', version: '3', mb: 48 },
  { name: 'city-block.glb', version: '7', mb: 210 },
]

const assetInput = (a: Asset) => ({
  name: a.name,
  version: a.version,
  content_hash: `sha256:${sha('sha256', `${a.name}@${a.version}`)}`,
})

interface Device {
  name: string
  /** Name the harness used before the device was renamed (alias). */
  oldName?: string
  renamedOnDay?: number
  fps: number
  /** Decode seconds per MB. */
  decode: number
  /** Base memory in MiB. */
  memory: number
  details(day: number): Json
}

const LAB_DEVICES: Device[] = [
  {
    name: 'pixel-8-lab-1',
    oldName: 'pixel8-lab1',
    renamedOnDay: 7,
    fps: 60,
    decode: 0.012,
    memory: 380,
    details: (day) => ({
      model: 'Google Pixel 8',
      os: day >= 20 ? 'Android 16' : 'Android 15',
      browser: 'Chrome 130',
      gpu: 'Mali-G715',
    }),
  },
  {
    name: 'iphone-15-lab-1',
    fps: 120,
    decode: 0.008,
    memory: 420,
    details: () => ({ model: 'iPhone 15 Pro', os: 'iOS 18.1', browser: 'Safari 18.1' }),
  },
  {
    name: 'macbook-m3-lab-1',
    fps: 120,
    decode: 0.004,
    memory: 650,
    details: () => ({
      model: 'MacBook Pro 14 (M3)',
      os: 'macOS 15.1',
      browser: 'Chrome 130',
    }),
  },
]

const CI_DEVICE = {
  name: 'ci-chrome-headless',
  details: { model: 'GitHub runner (4 vCPU)', os: 'Ubuntu 24.04', browser: 'Chrome 131 headless' },
}

const BANDWIDTHS = [20, 100]
const PIPELINES = ['draco', 'meshopt']

// Metrics as the harness defines them. Only raw, measured values.
const LOAD_METRICS: Record<string, Json> = {
  time_to_first_frame: {
    unit: 'ms',
    value_type: 'float',
    direction: 'lower_is_better',
    description: 'From opening the model until the first frame is shown.',
  },
  load_time: {
    unit: 's',
    value_type: 'float',
    direction: 'lower_is_better',
    description: 'Until the model is fully loaded. Not measured when loading does not finish.',
  },
  bytes_transferred: {
    unit: 'B',
    value_type: 'integer',
    direction: 'lower_is_better',
    description: 'Bytes received from the streaming server.',
  },
  peak_memory: {
    unit: 'MiB',
    value_type: 'float',
    direction: 'lower_is_better',
    description: 'Peak memory of the viewer process.',
  },
  frames_dropped: {
    unit: 'count',
    value_type: 'integer',
    direction: 'lower_is_better',
  },
  load_complete: {
    unit: '',
    value_type: 'boolean',
    direction: 'higher_is_better',
    description: 'Whether the model finished loading within the scenario duration.',
  },
  loaded: {
    unit: '%',
    value_type: 'float',
    direction: 'higher_is_better',
    description: 'Share of the model refined to full detail, over time.',
  },
  fps: {
    unit: 'fps',
    value_type: 'float',
    direction: 'higher_is_better',
    description: 'Frame rate over time.',
  },
}

const SMOKE_METRICS: Record<string, Json> = {
  time_to_first_frame: LOAD_METRICS.time_to_first_frame!,
  peak_memory: LOAD_METRICS.peak_memory!,
}

const DURATION_S = 40
const STEP_S = 0.25

function loadScenario(day: number) {
  return {
    name: 'progressive-load',
    definition: {
      description: 'Open a model, orbit the camera and wait for full detail.',
      camera_path: day >= 24 ? 'orbit-v2' : 'orbit',
      duration_s: DURATION_S,
      cache: 'cold',
    },
  }
}

const SMOKE_SCENARIO = {
  name: 'startup',
  definition: { steps: ['open-viewer', 'load-model', 'first-frame'], timeout_s: 10 },
}

function sharedEnvironment(day: number): Json {
  return {
    server: 'stream-server-01',
    region: 'eu-central',
    backend: day >= 15 ? 'hub 2.4.0' : 'hub 2.3.1',
    cache: 'cold',
  }
}

// ---------------------------------------------------------------------------
// Simulated measurements

interface LoadCase {
  asset: Asset
  bandwidth: number
  pipeline: string
  device: Device
}

interface Context {
  day: number
  /** Experiment build: faster decode of large models. */
  experiment?: boolean
  /** A re-measurement (correction): a cooler device, slightly faster. */
  remeasured?: boolean
}

function simulateSample(r: Random, c: LoadCase, ctx: Context, warmup: boolean, key: string) {
  const ratio = c.pipeline === 'draco' ? (ctx.day >= 22 ? 0.24 : 0.32) : 0.38
  const bytes = Math.round(c.asset.mb * 1e6 * ratio * (1 + 0.004 * r.normal()))
  const latency = (ctx.day >= 15 ? 0.11 : 0.13) * (1 + 0.1 * Math.abs(r.normal()))
  let decodeFactor = c.pipeline === 'draco' ? 1.3 : 0.8
  const regression = c.pipeline === 'meshopt' && ctx.day >= 12 && ctx.day <= 17
  if (regression) decodeFactor *= 3.5
  if (ctx.experiment && c.asset.mb > 100) decodeFactor *= 0.75
  const transfer = (bytes * 8) / (c.bandwidth * 1e6 * 0.85)
  const decode = (bytes / 1e6) * c.device.decode * decodeFactor
  let loadTime = (latency + transfer + decode) * Math.exp(0.03 * r.normal())
  if (warmup) loadTime *= 1.2
  if (ctx.remeasured) loadTime *= 0.96
  const ttffMs =
    (0.15 + latency + c.device.decode * 8 * decodeFactor) *
    1000 *
    Math.exp(0.05 * r.normal()) *
    (warmup ? 1.4 : 1)
  // The Pixel sometimes runs out of memory on the large model at low bandwidth.
  const complete =
    !(c.device.name.startsWith('pixel') && c.asset.mb > 100 && c.bandwidth === 20 && r.next() < 0.35) &&
    loadTime < DURATION_S - 2
  const cap = complete ? 100 : 78 + 15 * r.next()
  const ttffS = ttffMs / 1000

  const x: number[] = []
  const loaded: number[] = []
  const fps: number[] = []
  for (let i = 0; i * STEP_S <= DURATION_S + 1e-9; i++) {
    const t = round(i * STEP_S, 2)
    x.push(t)
    const frac =
      t < ttffS ? 0 : Math.min(1, (t - ttffS) / Math.max(0.1, (complete ? loadTime : loadTime * 1.4) - ttffS))
    loaded.push(round(Math.min(cap, 100 * frac ** 0.55), 2))
    let f: number
    if (t < ttffS) f = 0
    else if (frac < 1) f = c.device.fps * (0.45 + 0.35 * frac) * (regression ? 0.8 : 1)
    else f = c.device.fps * (0.97 + 0.03 * r.next())
    if (f > 0 && r.next() < 0.02) f *= 0.4 // GC pause or thermal dip
    fps.push(round(Math.max(0, f + (f > 0 ? r.normal() : 0)), 1))
  }

  const measurements: Record<string, Json> = {
    time_to_first_frame: round(ttffMs, 1),
    bytes_transferred: bytes,
    peak_memory: round(
      (c.device.memory + c.asset.mb * (c.pipeline === 'draco' ? 0.9 : 1.05)) * Math.exp(0.02 * r.normal()),
      1,
    ),
    frames_dropped: Math.max(0, Math.round(loadTime * 0.8 * decodeFactor + 2 * r.normal())),
    load_complete: complete,
  }
  if (complete) measurements.load_time = round(loadTime, 3)

  return {
    measurements,
    series: [
      { id: uuid(`${key}:loaded`), metric: 'loaded', x_dimension: 'time', x_unit: 's', x, y: loaded },
      { id: uuid(`${key}:fps`), metric: 'fps', x_dimension: 'time', x_unit: 's', x, y: fps },
    ],
  }
}

function loadCaseExecution(runKey: string, index: number, c: LoadCase, ctx: Context, start: Date) {
  const key = `${runKey}:ce:${c.asset.name}:${c.bandwidth}:${c.pipeline}:${c.device.name}${ctx.remeasured ? ':remeasured' : ''}`
  const r = random(key)
  const pipeline = c.pipeline === 'meshopt' && ctx.day < 5 ? 'meshopt-v1' : c.pipeline
  const deviceName =
    c.device.oldName && ctx.day < (c.device.renamedOnDay ?? 0) ? c.device.oldName : c.device.name
  const samples = [0, 1, 2, 3].map((repetition) => {
    const sampleKey = `${key}:sample:${repetition}`
    return {
      id: uuid(sampleKey),
      repetition,
      is_warmup: repetition === 0,
      ...simulateSample(r, c, ctx, repetition === 0, sampleKey),
    }
  })
  const started = new Date(start.getTime() + index * 95_000)
  return {
    id: uuid(key),
    body: {
      test_case: {
        scenario: loadScenario(ctx.day),
        assets: [assetInput(c.asset)],
        parameters: { bandwidth: c.bandwidth, pipeline },
        parameter_units: { bandwidth: 'Mbit/s' },
      },
      client_device: { name: deviceName, details: c.device.details(ctx.day) },
      started_at: iso(started),
      ended_at: iso(new Date(started.getTime() + 90_000)),
      metrics: LOAD_METRICS,
      samples,
    } satisfies Json,
  }
}

function nightlyMatrix(devices = LAB_DEVICES): LoadCase[] {
  return ASSETS.flatMap((asset) =>
    BANDWIDTHS.flatMap((bandwidth) =>
      PIPELINES.flatMap((pipeline) => devices.map((device) => ({ asset, bandwidth, pipeline, device }))),
    ),
  )
}

// ---------------------------------------------------------------------------
// Uploading

let token = ''
const auth = () => ({ authorization: `Bearer ${token}` })

interface RunPlan {
  key: string
  body: Json
  caseExecutions: Array<{ id: string; body: Json }>
  /** Upload only the first n case executions (failed run). */
  partial?: number
  close?: { status: 'completed' | 'failed'; ended_at: string }
}

async function uploadRun(plan: RunPlan): Promise<string> {
  const runId = uuid(plan.key)
  await call('PUT', `${API}/runs/${runId}`, plan.body, auth())
  const ces = plan.partial === undefined ? plan.caseExecutions : plan.caseExecutions.slice(0, plan.partial)
  await pool(
    CONCURRENCY,
    ces.map((ce) => () => call('PUT', `${API}/runs/${runId}/case-executions/${ce.id}`, ce.body, auth())),
  )
  if (plan.close) await call('POST', `${API}/runs/${runId}/close`, plan.close, auth())
  return runId
}

async function attach(runId: string, name: string, contentType: string, text: string) {
  const id = uuid(`${runId}:attachment:${name}`)
  const bytes = new TextEncoder().encode(text)
  await call(
    'PUT',
    `${API}/runs/${runId}/attachments/${id}?file_name=${encodeURIComponent(name)}`,
    bytes,
    { ...auth(), 'content-type': contentType, 'x-content-sha256': sha('sha256', text) },
  )
}

function nightlyPlan(day: number, options: { partial?: number } = {}): RunPlan {
  const key = `nightly-${day}`
  const start = at(day, 2, 0, Math.round(random(key).next() * 600))
  const caseExecutions = nightlyMatrix().map((c, i) => loadCaseExecution(key, i, c, { day }, start))
  const sut = mainSut(day)
  const failed = options.partial !== undefined
  return {
    key,
    body: {
      suite: {
        name: 'streaming-nightly',
        definition: { scenarios: ['progressive-load'], samples: 3, warmup: 1, devices: 'lab' },
      },
      sut_version: sut,
      shared_environment: sharedEnvironment(day),
      triggered_by: 'nightly',
      started_at: iso(start),
      tags: sut.tag ? ['nightly', 'release'] : ['nightly'],
    },
    caseExecutions,
    partial: options.partial,
    close: {
      status: failed ? 'failed' : 'completed',
      ended_at: iso(new Date(start.getTime() + (failed ? options.partial! : caseExecutions.length) * 95_000 + 60_000)),
    },
  }
}

const SMOKE_BRANCHES = [
  'feature/webgpu-renderer',
  'fix/meshopt-decoder',
  'feature/adaptive-lod',
  'chore/update-deps',
]

function smokePlan(day: number, n: number): RunPlan {
  const key = `smoke-${day}-${n}`
  const r = random(key)
  const branch = SMOKE_BRANCHES[(day + n) % SMOKE_BRANCHES.length]!
  const start = at(day, 9 + n * 4, Math.round(r.next() * 50))
  const webgpuFix = branch === 'feature/webgpu-renderer' && day >= 18
  const caseExecutions = ['webgl2', 'webgpu'].map((renderer, i) => {
    const ceKey = `${key}:ce:${renderer}`
    const cr = random(ceKey)
    const base = renderer === 'webgpu' ? (webgpuFix ? 380 : 520) : 450
    return {
      id: uuid(ceKey),
      body: {
        test_case: {
          scenario: SMOKE_SCENARIO,
          assets: [assetInput(ASSETS[0]!)],
          parameters: { renderer },
        },
        client_device: CI_DEVICE,
        started_at: iso(new Date(start.getTime() + i * 30_000)),
        ended_at: iso(new Date(start.getTime() + i * 30_000 + 25_000)),
        metrics: SMOKE_METRICS,
        samples: [0, 1, 2, 3, 4].map((repetition) => ({
          id: uuid(`${ceKey}:sample:${repetition}`),
          repetition,
          measurements: {
            time_to_first_frame: round(base * Math.exp(0.06 * cr.normal()), 1),
            peak_memory: round((renderer === 'webgpu' ? 310 : 290) * Math.exp(0.03 * cr.normal()), 1),
          },
        })),
      } satisfies Json,
    }
  })
  return {
    key,
    body: {
      suite: { name: 'smoke', definition: { scenarios: ['startup'], samples: 5 } },
      sut_version: {
        commit: sha('sha1', `${branch}-${day}-${n}`),
        branch,
        tag: null,
        build_config: 'debug',
      },
      shared_environment: { server: 'ci-stream-server', region: 'github-actions' },
      triggered_by: 'ci',
      started_at: iso(start),
      tags: ['ci'],
    },
    caseExecutions,
    close: { status: 'completed', ended_at: iso(new Date(start.getTime() + 70_000)) },
  }
}

// ---------------------------------------------------------------------------

async function main() {
  console.log(`Seeding ${UI} and ${API}`)
  const marker = uuid('nightly-0')
  const existing = (await call('GET', `${UI}/runs?run_id=${marker}`)) as { runs: Json[] }
  if (existing.runs.length > 0) {
    console.error('This database is already seeded. Start from an empty one to seed again.')
    process.exit(1)
  }

  const created = (await call('POST', `${UI}/tokens`, { name: 'seed script', scope: 'write' })) as {
    id: string
    secret: string
  }
  token = created.secret
  try {
    await seed()
  } finally {
    await call('POST', `${UI}/tokens/${created.id}/revoke`)
  }
  console.log('Done.')
}

async function seed() {
  const FAILED_DAY = 9
  const INVALID_DAY = 13
  const CORRECTED_DAY = 27
  const runIds = new Map<string, string>()
  const started = Date.now()

  // Nightly history, oldest first.
  for (let day = 0; day < DAYS; day++) {
    const plan = nightlyPlan(day, day === FAILED_DAY ? { partial: 10 } : {})
    runIds.set(plan.key, await uploadRun(plan))
    // CI smoke runs on weekdays.
    if (!isWeekend(day)) {
      for (let n = 0; n < (day % 3 === 0 ? 2 : 1); n++) {
        const smoke = smokePlan(day, n)
        runIds.set(smoke.key, await uploadRun(smoke))
      }
    }
    console.log(`  day ${String(day).padStart(2)} ${at(day).toISOString().slice(0, 10)}  (${Math.round((Date.now() - started) / 1000)} s)`)
  }

  // Experiment build on the MacBook, a manual run.
  const expKey = 'experiment-adaptive-lod'
  const expStart = at(26, 14, 30)
  const experiment = await uploadRun({
    key: expKey,
    body: {
      suite: {
        name: 'streaming-nightly',
        definition: { scenarios: ['progressive-load'], samples: 3, warmup: 1, devices: 'lab' },
      },
      sut_version: {
        commit: sha('sha1', 'feature/adaptive-lod-26'),
        branch: 'feature/adaptive-lod',
        tag: null,
        build_config: 'release',
      },
      shared_environment: sharedEnvironment(26),
      triggered_by: 'manual',
      started_at: iso(expStart),
      tags: ['experiment'],
    },
    caseExecutions: nightlyMatrix([LAB_DEVICES[2]!]).map((c, i) =>
      loadCaseExecution(expKey, i, c, { day: 26, experiment: true }, expStart),
    ),
    close: { status: 'completed', ended_at: iso(new Date(expStart.getTime() + 8 * 95_000)) },
  })
  console.log('  experiment run')

  // A correction: one case execution of a nightly run measured again.
  const correctedRun = runIds.get(`nightly-${CORRECTED_DAY}`)!
  const corrected = nightlyMatrix().findIndex(
    (c) => c.asset.mb > 100 && c.bandwidth === 100 && c.pipeline === 'draco' && c.device.name.startsWith('iphone'),
  )
  const original = nightlyPlan(CORRECTED_DAY).caseExecutions[corrected]!
  const correctionStart = at(CORRECTED_DAY, 9, 15)
  const correction = loadCaseExecution(
    `nightly-${CORRECTED_DAY}`,
    0,
    nightlyMatrix()[corrected]!,
    { day: CORRECTED_DAY, remeasured: true },
    correctionStart,
  )
  await call(
    'PUT',
    `${API}/runs/${correctedRun}/case-executions/${correction.id}`,
    { ...(correction.body as Record<string, Json>), supersedes_id: original.id },
    auth(),
  )
  console.log('  correction')

  // A run that is never closed: `running` until the stale timeout aborts it.
  const openKey = 'open-manual-run'
  const openStart = at(DAYS, 21, 0)
  await uploadRun({
    key: openKey,
    body: {
      suite: {
        name: 'streaming-nightly',
        definition: { scenarios: ['progressive-load'], samples: 3, warmup: 1, devices: 'lab' },
      },
      sut_version: mainSut(DAYS - 1),
      shared_environment: sharedEnvironment(DAYS - 1),
      triggered_by: 'manual',
      started_at: iso(openStart),
      tags: ['manual'],
    },
    caseExecutions: nightlyMatrix()
      .slice(0, 3)
      .map((c, i) => loadCaseExecution(openKey, i, c, { day: DAYS - 1 }, openStart)),
  })
  console.log('  open run')

  // Attachments.
  const failedRun = runIds.get(`nightly-${FAILED_DAY}`)!
  const latestRun = runIds.get(`nightly-${DAYS - 1}`)!
  await attach(
    failedRun,
    'harness.log',
    'text/plain',
    [
      `${iso(at(FAILED_DAY, 2, 0, 12))} INFO  starting suite streaming-nightly (24 case executions)`,
      `${iso(at(FAILED_DAY, 2, 15, 40))} INFO  case 10/24 done`,
      `${iso(at(FAILED_DAY, 2, 16, 3))} ERROR adb: device 'pixel-8-lab-1' not found`,
      `${iso(at(FAILED_DAY, 2, 16, 3))} ERROR lost connection to device, aborting suite`,
      '',
    ].join('\n'),
  )
  await attach(
    latestRun,
    'environment.json',
    'application/json',
    JSON.stringify({ harness: 'bench-harness 0.9.2', node: 'lab-runner-02', network: 'shaped (tc netem)' }, null, 2),
  )
  await attach(
    latestRun,
    'summary.md',
    'text/markdown',
    '# Nightly summary\n\nAll 24 case executions passed. City block on the Pixel at 20 Mbit/s did not finish loading in some samples.\n',
  )
  console.log('  attachments')

  // UI-only operations: invalidation, aliases, derived metrics.
  await call('POST', `${UI}/runs/${runIds.get(`nightly-${INVALID_DAY}`)!}/invalidate`, {
    reason: 'Lab network was throttled by a firewall update; results are not representative.',
  })
  await call('PUT', `${UI}/aliases`, {
    dimension: 'client_device',
    source: 'pixel8-lab1',
    target: 'pixel-8-lab-1',
  })
  await call('PUT', `${UI}/aliases`, {
    dimension: 'parameter:pipeline',
    source: 'meshopt-v1',
    target: 'meshopt',
  })
  const derived: Array<[string, Json]> = [
    ['throughput', { expression: 'bytes_transferred / load_time', unit: 'MB/s', description: 'Average download rate while loading.' }],
    // A second version of the same metric.
    ['throughput', { expression: 'bytes_transferred / load_time', unit: 'Mbit/s', description: 'Average download rate while loading, comparable to the link bandwidth.' }],
    ['time_to_90', { expression: 'time_to(series.loaded, 90)', unit: 's', description: 'Until 90 % of the model is at full detail.' }],
    ['time_to_full', { expression: 'time_to(series.loaded, 99.9)', unit: 's', description: 'Until the whole model is at full detail. No value when loading does not finish.' }],
    ['steady_fps', { expression: 'mean(series.fps, from=derived.time_to_90)', unit: 'fps', description: 'Mean frame rate once 90 % is loaded.' }],
    ['load_stall', { expression: 'frames_dropped / load_time', unit: 'count/s', description: 'Dropped frames per second of loading.' }],
  ]
  for (const [name, body] of derived) await call('PUT', `${UI}/derived-metrics/${name}`, body)
  console.log('  invalidation, aliases, derived metrics')

  // Tags and annotations.
  await call('PUT', `${UI}/runs/${latestRun}/tags/baseline`)
  await call('PUT', `${UI}/runs/${experiment}/tags/adaptive-lod`)
  const testCases = (await call('GET', `${UI}/test-cases`)) as Array<{
    id: number
    assets: Array<{ name: string }>
  }>
  const heavy = testCases.filter((t) => t.assets.some((a) => a.name === 'city-block.glb'))
  for (const t of heavy) await call('PUT', `${UI}/test-cases/${t.id}/tags/heavy`)
  const annotations: Json[] = [
    { run_id: failedRun, body: 'Harness lost the USB connection to the Pixel; the remaining cases were not run.' },
    { run_id: experiment, body: 'Adaptive LOD prototype: decode large meshes in smaller batches. Compare with the nightly of the same day.' },
    { occurred_at: iso(at(11, 10)), body: 'meshopt decoder 0.21 merged (later found to decode 3.5 times slower).' },
    { occurred_at: iso(at(14, 16)), body: 'Streaming backend upgraded to hub 2.4.0.' },
    { occurred_at: iso(at(17, 11)), body: 'meshopt decoder regression fixed.' },
    { occurred_at: iso(at(21, 15)), body: 'Switched to new draco compression settings.' },
    { occurred_at: iso(at(23, 15)), body: 'New camera path (orbit-v2); progressive-load trends start over.' },
  ]
  if (heavy[0]) annotations.push({ test_case_id: heavy[0].id, body: 'Large model; known to run out of memory on the Pixel at 20 Mbit/s.' })
  for (const a of annotations) await call('POST', `${UI}/annotations`, a)
  console.log('  tags, annotations')

  describe(runIds.size + 2)
}

function describe(runs: number) {
  console.log(`
Created ${runs} runs:
  - streaming-nightly, ${DAYS} nightly runs ${iso(at(0)).slice(0, 10)} .. ${iso(at(DAYS - 1)).slice(0, 10)}
    (day 9 failed with partial data, day 13 invalid, day 27 has a correction)
  - smoke (CI) runs on feature branches, an experiment run (manual, tag
    'experiment') and a run left open (aborted after the stale timeout)
  - aliases: device pixel8-lab1 -> pixel-8-lab-1, pipeline meshopt-v1 -> meshopt
  - derived metrics: throughput (2 versions), time_to_90, time_to_full,
    steady_fps (uses derived.time_to_90), load_stall`)
}

await main()

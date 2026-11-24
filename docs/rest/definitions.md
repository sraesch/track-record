# Definitions

The definitions and dimensions that uploads create: suites, scenarios, test
cases, metrics, client devices, environments, SUT versions and assets, plus
the unit catalogue and the aliases. Every endpoint on this page works with a
token of any scope and returns the whole list (no paging).

None of these can be created or changed directly through the machine API.
Uploads create them ([Ingestion](ingestion.md)); metrics and aliases are
changed in the web UI.

## Suites

```http
GET /api/v1/suites
```

Suites with all their versions, by name. Response: `VersionedDefinition`[]:

| Field | Type | Meaning |
|---|---|---|
| `id` | integer | Suite id |
| `name` | string | Suite name |
| `versions` | `DefinitionVersion`[] | By version number |

`DefinitionVersion`:

| Field | Type | Meaning |
|---|---|---|
| `id` | integer | Id of this version (`suite_version_id` in runs) |
| `version` | integer | Version number, from 1 |
| `definition` | any JSON | The definition as uploaded |
| `created_at` | timestamp | When it was first uploaded |

```json
[
  {
    "id": 1,
    "name": "web-viewer",
    "versions": [
      {"id": 1, "version": 1, "definition": {"scenarios": ["load-model"]}, "created_at": "2026-11-23T19:57:13.347379Z"}
    ]
  }
]
```

## Scenarios

```http
GET /api/v1/scenarios
```

Scenarios with all their versions, by name; the same shape as
[suites](#suites). The version `id` is the `scenario_version_id` of test
cases. A changed scenario definition is a new version and therefore a
different case: trends never mix versions.

## List test cases

```http
GET /api/v1/test-cases
```

| Parameter | Type | Meaning |
|---|---|---|
| `scenario` | string | Only test cases of this scenario (all versions) |

Sorted by scenario name, scenario version and id. Response:
`TestCaseSummary`[]:

| Field | Type | Meaning |
|---|---|---|
| `id` | integer | Test case id |
| `scenario` | string | Scenario name |
| `scenario_version` | integer | Scenario version number |
| `scenario_version_id` | integer | Scenario version id |
| `parameters` | object | Parameters as uploaded |
| `resolved_parameters` | object | Parameters after [aliases](#aliases) (FR-14) |
| `parameter_units` | object | Units of numeric parameters |
| `assets` | `AssetRef`[] | `id`, `name`, `version`, `content_hash` |
| `tags` | string[] | Tags of the test case, sorted; see [tags](annotations.md#tag-a-test-case) |

```json
[
  {
    "id": 1,
    "scenario": "load-model",
    "scenario_version": 1,
    "scenario_version_id": 1,
    "parameters": {"bandwidth": 100, "codec": "draco", "progressive": true},
    "resolved_parameters": {"bandwidth": 100, "codec": "draco", "progressive": true},
    "parameter_units": {"bandwidth": "Mbit/s"},
    "assets": [{"id": 1, "name": "engine.glb", "version": "3", "content_hash": null}],
    "tags": []
  }
]
```

## Get a test case

```http
GET /api/v1/test-cases/{test_case_id}
```

Response: one `TestCaseSummary`. `404 not_found` if it does not exist.

## Metrics

```http
GET /api/v1/metrics
```

All uploaded metrics, by name. A metric has one definition and no versions
(FR-7, [ADR 0018](../adr/0018-metrics-without-versions.md)); the web UI can
change it. Response: `Metric`[]:

| Field | Type | Meaning |
|---|---|---|
| `id` | integer | Metric id |
| `name` | string | Name |
| `unit` | string | Unit symbol; `""` for dimensionless |
| `value_type` | `float` \| `integer` \| `boolean` | |
| `direction` | `higher_is_better` \| `lower_is_better` | |
| `description` | string | |
| `created_at` | timestamp | When it was created |
| `updated_at` | timestamp | When the definition was last changed |
| `has_results` | boolean | Measurements or series of the metric are stored |
| `used_by` | string[] | Names of the derived metrics whose expressions refer to it |

```json
[
  {
    "id": 3,
    "name": "load_time",
    "unit": "ms",
    "value_type": "float",
    "direction": "lower_is_better",
    "description": "Time until the model is fully loaded",
    "created_at": "2026-11-23T19:57:13.401757Z",
    "updated_at": "2026-11-23T19:57:13.401757Z",
    "has_results": true,
    "used_by": []
  }
]
```

A harness can read this list to send the current definitions with its
uploads and so avoid `409 metric_definition_conflict` after a metric was
changed in the web UI.

## Units

```http
GET /api/v1/units
```

The predefined unit catalogue (FR-13a), sorted by quantity name, then by
size. Response: `Unit`[]:

| Field | Type | Meaning |
|---|---|---|
| `symbol` | string | What uploads and expressions write, e.g. `ms` |
| `quantity` | string | Units of the same quantity convert into each other |
| `to_base` | number | A value in this unit times `to_base` is the value in the quantity's base unit |
| `name` | string | Full name |

```json
[
  {"symbol": "count", "quantity": "count", "to_base": 1.0, "name": "count"},
  {"symbol": "thousand", "quantity": "count", "to_base": 1000.0, "name": "thousand"},
  {"symbol": "million", "quantity": "count", "to_base": 1000000.0, "name": "million"},
  "..."
]
```

The catalogue of release 0.1.0 (the endpoint is authoritative):

| Quantity | Symbols (base unit first in bold) |
|---|---|
| time | `ns`, `us`, `µs`, `ms`, **`s`**, `min`, `h`, `d` |
| data size | `bit`, `kbit`, `Mbit`, `Gbit`, **`B`**, `kB`, `MB`, `GB`, `TB`, `KiB`, `MiB`, `GiB`, `TiB` |
| data rate | `bit/s`, `kbit/s`, `Mbit/s`, `Gbit/s`, **`B/s`**, `kB/s`, `MB/s`, `GB/s`, `KiB/s`, `MiB/s`, `GiB/s` |
| frequency | **`Hz`**, `kHz`, `MHz`, `GHz` |
| frame rate | **`fps`** |
| ratio | **`""`** (dimensionless), `%`, `‰`, `ppm` |
| count | **`count`**, `thousand`, `million`, `billion` |
| length | `mm`, `cm`, **`m`**, `km` |
| pixels | **`px`**, `Mpx` |
| power | `mW`, **`W`**, `kW` |
| energy | `mJ`, **`J`**, `kJ`, `mWh`, `Wh`, `kWh` |

Units outside the catalogue are allowed for metrics, series and parameters
but do not convert to anything. In [expressions](derived-metrics.md#units),
products and quotients of catalogue units (`B/ms`) convert as well.

## Client devices

```http
GET /api/v1/client-devices
```

All client devices, by name. Response: `ClientDevice`[]:

| Field | Type | Meaning |
|---|---|---|
| `id` | integer | Device id |
| `name` | string | Stable name as uploaded |
| `resolved_name` | string | Name after [aliases](#aliases). Filters, trends and comparisons use this |
| `created_at` | timestamp | First upload |

```json
[
  {"id": 4, "name": "iphone-15-lab-2", "resolved_name": "iphone-15-lab-2", "created_at": "2026-11-23T19:57:13.539826Z"},
  {"id": 1, "name": "pixel-8-lab-1", "resolved_name": "pixel-8-lab-1", "created_at": "2026-11-23T19:57:13.401757Z"}
]
```

## Environments

```http
GET /api/v1/environments/{environment_id}
```

A shared environment (`shared_environment_id` of a run) or the device
details of a case execution (`device_environment_id`). Both are stored the
same way: identical details once, identified by their fingerprint.

Response `Environment`:

| Field | Type | Meaning |
|---|---|---|
| `id` | integer | Environment id |
| `details` | object | The details as uploaded |
| `fingerprint` | string | SHA-256 of the details, hex |
| `created_at` | timestamp | First upload |

```json
{
  "id": 1,
  "details": {"backend_version": "2.4.1", "server": "render-01"},
  "fingerprint": "1a74c726b0932ca088b0654e9b405285577f9e920d00476383504deacf946a22",
  "created_at": "2026-11-23T19:57:13.347379Z"
}
```

`404 not_found` if it does not exist.

## SUT versions

```http
GET /api/v1/sut-versions
```

All versions of the system under test, newest first. Response:
`SutVersion`[]: `id`, `commit`, `branch`, `build_config`, `tag` (the last
three may be `null`).

```json
[
  {"id": 4, "commit": "8be71d0", "branch": "main", "build_config": "release", "tag": null},
  {"id": 1, "commit": "4f2a9c1", "branch": "main", "build_config": "release", "tag": null}
]
```

## Assets

```http
GET /api/v1/assets
```

All assets, by name. Response: `Asset`[]: `id`, `name`, `version`,
`content_hash` (either may be `null`, not both) and `created_at`.

```json
[
  {"id": 1, "name": "engine.glb", "version": "3", "content_hash": null, "created_at": "2026-11-23T19:57:13.401757Z"}
]
```

## Aliases

```http
GET /api/v1/aliases
```

Aliases with their current target (FR-14), by dimension and source. An
alias maps a client device name or a parameter value to another one, so
that results uploaded under both form one trend. Aliases are set in the web
UI and versioned; this endpoint shows the latest version of each.

Response: `Alias`[]:

| Field | Type | Meaning |
|---|---|---|
| `id` | integer | Alias id |
| `dimension` | string | `client_device`, or `parameter:<name>` for the values of one parameter |
| `source` | JSON scalar | The value that is mapped: a device name, or a parameter value with its JSON type |
| `target` | JSON scalar \| null | What it maps to; `null` if the alias was removed |
| `version` | integer | Current version |
| `updated_at` | timestamp | When the current version was set |

```json
[
  {"id": 1, "dimension": "client_device", "source": "pixel-8-lab-old", "target": "pixel-8-lab-1", "version": 1, "updated_at": "2026-11-22T09:12:00Z"},
  {"id": 2, "dimension": "parameter:codec", "source": "draco-1.5", "target": "draco", "version": 2, "updated_at": "2026-11-22T09:14:30Z"}
]
```

Aliases apply in every endpoint: device and parameter filters match the
values after aliases, analysis groups by them, and responses show both the
uploaded and the resolved value (`name`/`resolved_name`,
`parameters`/`resolved_parameters`).

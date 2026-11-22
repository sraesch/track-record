// Uploaded JSON objects (parameters, environment details) are typed as
// plain objects in the generated API types; these helpers read them.

/** A JSON object from the API as a record of JSON values. */
export function jsonObject(value: object | null | undefined) {
  return (value ?? {}) as Record<string, unknown>
}

/**
 * Nested objects flattened to dotted keys, e.g. `{ gpu: { driver: "1" } }`
 * to `[["gpu.driver", "1"]]`. Arrays, scalars and empty objects are
 * leaves.
 */
export function flattenJson(
  value: Record<string, unknown>,
  prefix = '',
): [string, unknown][] {
  return Object.entries(value).flatMap(([key, child]) => {
    const path = prefix ? `${prefix}.${key}` : key
    const nested =
      child !== null &&
      typeof child === 'object' &&
      !Array.isArray(child) &&
      Object.keys(child).length > 0
    return nested
      ? flattenJson(child as Record<string, unknown>, path)
      : [[path, child] as [string, unknown]]
  })
}

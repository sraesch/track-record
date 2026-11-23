export type ClassValue = string | false | null | undefined;

/** Join class names, skipping falsy values. */
export function cx(...values: readonly ClassValue[]): string {
  return values.filter((v): v is string => typeof v === 'string' && v.length > 0).join(' ');
}

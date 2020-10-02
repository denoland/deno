// Helper function: setExpiration()
// returns the number of seconds since January 1, 1970, 00:00:00 UTC
export function setExpiration(exp: number | Date): number {
  return Math.round(
    (exp instanceof Date ? exp.getTime() : Date.now() + exp * 1000) / 1000,
  );
}

export function isObject(obj: unknown): obj is object {
  return (
    obj !== null && typeof obj === "object" && Array.isArray(obj) === false
  );
}

export function hasProperty<K extends string>(
  key: K,
  x: object,
): x is { [key in K]: unknown } {
  return key in x;
}

export function isExpired(exp: number, leeway = 0): boolean {
  return exp + leeway < Date.now() / 1000;
}

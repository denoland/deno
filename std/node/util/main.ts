export function isArray(value: unknown): boolean {
  return Array.isArray(value);
}

export function isBoolean(value: unknown): boolean {
  return typeof value === "boolean" || value instanceof Boolean;
}

export function isNull(value: unknown): boolean {
  return value === null;
}

export function isNullOrUndefined(value: unknown): boolean {
  return value === null || value === undefined;
}

export function isNumber(value: unknown): boolean {
  return typeof value === "number" || value instanceof Number;
}

export function isString(value: unknown): boolean {
  return typeof value === "string" || value instanceof String;
}

export function isSymbol(value: unknown): boolean {
  return typeof value === "symbol";
}

export function isUndefined(value: unknown): boolean {
  return value === undefined;
}

export function isObject(value: unknown): boolean {
  return value !== null && typeof value === "object";
}

export function isError(e: unknown): boolean {
  return e instanceof Error;
}

export function isFunction(value: unknown): boolean {
  return typeof value === "function";
}

export function isRegExp(value: unknown): boolean {
  return value instanceof RegExp;
}

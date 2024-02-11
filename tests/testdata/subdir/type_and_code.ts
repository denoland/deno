export interface AnInterface {
  a: string;
}

export function isAnInterface(value: unknown): value is AnInterface {
  return value && typeof value === "object" && "a" in value;
}

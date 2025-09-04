export function resolve(value: string): string {
  return import.meta.resolve(value);
}

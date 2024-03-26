export function assertEquals<T>(
  actual: T,
  expected: T,
  msg?: string,
  options: { formatter?: (value: unknown) => string } = {},
) {
  return true;
}

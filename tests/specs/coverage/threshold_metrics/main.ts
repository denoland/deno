export function add(a: number, b: number): number {
  return a + b;
}

export function classify(n: number): string {
  if (n > 0) {
    return "positive";
  } else {
    return "non-positive";
  }
}

export function unused(): string {
  return "never called";
}

// Function without explicit return type — triggers slow-type diagnostic.
export function compute(x: number) {
  return x * 2;
}

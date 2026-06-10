export function addB(a: number, b: number) {
  return a + b;
}

export * from "./d.ts";
export * from "./non-existent.ts"; // should not cause a linting error

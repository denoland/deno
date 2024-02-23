import * as test from "@denotest/esm-basic";

export function add(a: number, b: number): number {
  return a + b;
}

export function getValue(): any {
  return test.getValue();
}

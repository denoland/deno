import * as test from "@denotest/esm-basic";

export function add(a: number, b: number) {
  return a + b;
}

export function getValue() {
  return test.getValue();
}

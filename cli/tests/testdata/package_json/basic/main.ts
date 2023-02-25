import * as test from "@denotest/esm-basic";

console.log(test.getValue());

export function add(a: number, b: number) {
  return a + b;
}

export function getValue() {
  return test.getValue();
}

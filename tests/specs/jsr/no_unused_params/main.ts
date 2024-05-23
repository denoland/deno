import * as inner from "jsr:@denotest/add@1";

export function add(a: number, b: number): number {
  return inner.add(a, b);
}

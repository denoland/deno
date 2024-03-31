import * as inner from "jsr:@denotest/add";

export function add(a: number, b: number): number {
  return inner.add(a, b);
}

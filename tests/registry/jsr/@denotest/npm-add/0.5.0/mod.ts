import * as npmAdd from "npm:@denotest/add@0.5";

export function sum(a: number, b: number): number {
  return npmAdd.sum(a, b);
}

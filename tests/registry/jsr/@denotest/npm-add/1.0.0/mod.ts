import * as npmAdd from "npm:@denotest/add@1";

export function add(a: number, b: number): number {
  return npmAdd.add(a, b);
}

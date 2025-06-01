// test using a bare specifier to a pkg.json dep
import { add } from "add";

export function subtract(a: number, b: number): number {
  return add(a, -b);
}

import { add } from "../add/mod.ts";

export function subtract(a: number, b: number): number {
  return add(a, -1 * b);
}

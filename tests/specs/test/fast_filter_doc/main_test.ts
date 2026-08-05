/**
 * ```ts
 * import { add } from "./main_test.ts";
 * if (add(1, 2) !== 3) throw new Error("bad add");
 * ```
 */
export function add(a: number, b: number): number {
  return a + b;
}

Deno.test("match", () => {});
Deno.test("other", () => {});

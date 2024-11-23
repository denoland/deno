/**
 * ```ts
 * import { assertAlmostEquals } from "@std/assert/almost-equals";
 *
 * const x = sub(3, 1);
 * const y = div(5, x);
 * assertAlmostEquals(y, 2.0); // throws
 * ```
 * @module doc
 */

/**
 * ```ts
 * import { assertEquals } from "@std/assert/equals";
 *
 * assertEquals(div(6, 2), 4); // throws
 * ```
 */
export function div(a: number, b: number): number {
  return a / b;
}

/**
 * ```ts
 * import { assertEquals } from "@std/assert/equals";
 *
 * assertEquals(sub(6, 2), 3); // throws
 * ```
 */
const sub = (a: number, b: number): number => a - b;

export { sub };

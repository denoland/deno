/**
 * ```ts
 * import { assertEquals } from "@std/assert/equals";
 *
 * const foo = createFoo(3);
 * assertEquals(foo, 9);
 * ```
 */
export function createFoo(x: number): number {
  return x * x;
}

export const foo = 42;

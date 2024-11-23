export {};

declare global {
  /**
   * ```ts
   * import { assertEquals } from "@std/assert/equals";
   * import "./mod.js";
   *
   * assertEquals(myFunction(1, 2), 3);
   * ```
   */
  export function myFunction(a: number, b: number): number;
}

declare namespace MyNamespace {
  /**
   * ```ts
   * import { assertEquals } from "@std/assert/equals";
   * import "./mod.js";
   *
   * assertEquals(MyNamespace.add(1, 2), 3);
   * ```
   */
  export function add(a: number, b: number): number;
}

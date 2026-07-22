/**
 * ```ts
 * import { add } from "./mod.ts";
 *
 * add(1, 2);
 *
 * // @ts-expect-error: can only add numbers
 * add('1', '2');
 * ```
 */
export function add(first: number, second: number) {
  return first + second;
}

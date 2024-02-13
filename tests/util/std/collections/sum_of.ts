// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Applies the given selector to all elements in the given collection and
 * calculates the sum of the results.
 *
 * @example
 * ```ts
 * import { sumOf } from "https://deno.land/std@$STD_VERSION/collections/sum_of.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const people = [
 *   { name: "Anna", age: 34 },
 *   { name: "Kim", age: 42 },
 *   { name: "John", age: 23 },
 * ];
 * const totalAge = sumOf(people, (i) => i.age);
 *
 * assertEquals(totalAge, 99);
 * ```
 */
export function sumOf<T>(
  array: Iterable<T>,
  selector: (el: T) => number,
): number {
  let sum = 0;

  for (const i of array) {
    sum += selector(i);
  }

  return sum;
}

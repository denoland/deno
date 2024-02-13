// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Returns the first element that is the smallest value of the given function or
 * undefined if there are no elements
 *
 * @example
 * ```ts
 * import { minBy } from "https://deno.land/std@$STD_VERSION/collections/min_by.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const people = [
 *   { name: "Anna", age: 34 },
 *   { name: "Kim", age: 42 },
 *   { name: "John", age: 23 },
 * ];
 *
 * const personWithMinAge = minBy(people, (i) => i.age);
 *
 * assertEquals(personWithMinAge, { name: "John", age: 23 });
 * ```
 */
export function minBy<T>(
  array: Iterable<T>,
  selector: (el: T) => number,
): T | undefined;
/**
 * Returns the first element that is the smallest value of the given function or
 * undefined if there are no elements
 *
 * @example
 * ```ts
 * import { minBy } from "https://deno.land/std@$STD_VERSION/collections/min_by.ts";
 *
 * const people = [
 *   { name: "Anna" },
 *   { name: "Kim" },
 *   { name: "John" },
 * ];
 *
 * const personWithMinName = minBy(people, (i) => i.name);
 * ```
 */
export function minBy<T>(
  array: Iterable<T>,
  selector: (el: T) => string,
): T | undefined;
/**
 * Returns the first element that is the smallest value of the given function or
 * undefined if there are no elements
 *
 * @example
 * ```ts
 * import { minBy } from "https://deno.land/std@$STD_VERSION/collections/min_by.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const people = [
 *   { name: "Anna", age: 34n },
 *   { name: "Kim", age: 42n },
 *   { name: "John", age: 23n },
 * ];
 *
 * const personWithMinAge = minBy(people, (i) => i.age);
 *
 * assertEquals(personWithMinAge, { name: "John", age: 23n });
 * ```
 */
export function minBy<T>(
  array: Iterable<T>,
  selector: (el: T) => bigint,
): T | undefined;
/**
 * Returns the first element that is the smallest value of the given function or
 * undefined if there are no elements
 *
 * @example
 * ```ts
 * import { minBy } from "https://deno.land/std@$STD_VERSION/collections/min_by.ts";
 *
 * const people = [
 *   { name: "Anna", startedAt: new Date("2020-01-01") },
 *   { name: "Kim", startedAt: new Date("2020-03-01") },
 *   { name: "John", startedAt: new Date("2019-01-01") },
 * ];
 *
 * const personWithMinStartedAt = minBy(people, (i) => i.startedAt);
 * ```
 */
export function minBy<T>(
  array: Iterable<T>,
  selector: (el: T) => Date,
): T | undefined;
export function minBy<T>(
  array: Iterable<T>,
  selector:
    | ((el: T) => number)
    | ((el: T) => string)
    | ((el: T) => bigint)
    | ((el: T) => Date),
): T | undefined {
  let min: T | undefined = undefined;
  let minValue: ReturnType<typeof selector> | undefined = undefined;

  for (const current of array) {
    const currentValue = selector(current);

    if (minValue === undefined || currentValue < minValue) {
      min = current;
      minValue = currentValue;
    }
  }

  return min;
}

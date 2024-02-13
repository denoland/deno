// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Returns the first element that is the largest value of the given function or
 * undefined if there are no elements.
 *
 * @example
 * ```ts
 * import { maxBy } from "https://deno.land/std@$STD_VERSION/collections/max_by.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const people = [
 *   { name: "Anna", age: 34 },
 *   { name: "Kim", age: 42 },
 *   { name: "John", age: 23 },
 * ];
 *
 * const personWithMaxAge = maxBy(people, (i) => i.age);
 *
 * assertEquals(personWithMaxAge, { name: "Kim", age: 42 });
 * ```
 */
export function maxBy<T>(
  array: Iterable<T>,
  selector: (el: T) => number,
): T | undefined;
/**
 * Returns the first element that is the largest value of the given function or
 * undefined if there are no elements.
 *
 * @example
 * ```ts
 * import { maxBy } from "https://deno.land/std@$STD_VERSION/collections/max_by.ts";
 *
 * const people = [
 *   { name: "Anna" },
 *   { name: "Kim" },
 *   { name: "John" },
 * ];
 *
 * const personWithMaxName = maxBy(people, (i) => i.name);
 * ```
 */
export function maxBy<T>(
  array: Iterable<T>,
  selector: (el: T) => string,
): T | undefined;
/**
 * Returns the first element that is the largest value of the given function or
 * undefined if there are no elements.
 *
 * @example
 * ```ts
 * import { maxBy } from "https://deno.land/std@$STD_VERSION/collections/max_by.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const people = [
 *   { name: "Anna", age: 34n },
 *   { name: "Kim", age: 42n },
 *   { name: "John", age: 23n },
 * ];
 *
 * const personWithMaxAge = maxBy(people, (i) => i.age);
 *
 * assertEquals(personWithMaxAge, { name: "Kim", age: 42n });
 * ```
 */
export function maxBy<T>(
  array: Iterable<T>,
  selector: (el: T) => bigint,
): T | undefined;
/**
 * Returns the first element that is the largest value of the given function or
 * undefined if there are no elements.
 *
 * @example
 * ```ts
 * import { maxBy } from "https://deno.land/std@$STD_VERSION/collections/max_by.ts";
 *
 * const people = [
 *   { name: "Anna", startedAt: new Date("2020-01-01") },
 *   { name: "Kim", startedAt: new Date("2021-03-01") },
 *   { name: "John", startedAt: new Date("2020-03-01") },
 * ];
 *
 * const personWithLastStartedAt = maxBy(people, (i) => i.startedAt);
 * ```
 */
export function maxBy<T>(
  array: Iterable<T>,
  selector: (el: T) => Date,
): T | undefined;
export function maxBy<T>(
  array: Iterable<T>,
  selector:
    | ((el: T) => number)
    | ((el: T) => string)
    | ((el: T) => bigint)
    | ((el: T) => Date),
): T | undefined {
  let max: T | undefined = undefined;
  let maxValue: ReturnType<typeof selector> | undefined = undefined;

  for (const current of array) {
    const currentValue = selector(current);

    if (maxValue === undefined || currentValue > maxValue) {
      max = current;
      maxValue = currentValue;
    }
  }

  return max;
}

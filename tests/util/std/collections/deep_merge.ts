// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { filterInPlace } from "./_utils.ts";

const { hasOwn } = Object;

/**
 * Merges the two given Records, recursively merging any nested Records with the
 * second collection overriding the first in case of conflict
 *
 * For arrays, maps and sets, a merging strategy can be specified to either
 * `replace` values, or `merge` them instead. Use `includeNonEnumerable` option
 * to include non-enumerable properties too.
 *
 * @example
 * ```ts
 * import { deepMerge } from "https://deno.land/std@$STD_VERSION/collections/deep_merge.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const a = { foo: true };
 * const b = { foo: { bar: true } };
 *
 * assertEquals(deepMerge(a, b), { foo: { bar: true } });
 * ```
 */
export function deepMerge<
  T extends Record<PropertyKey, unknown>,
>(
  record: Partial<Readonly<T>>,
  other: Partial<Readonly<T>>,
  options?: Readonly<DeepMergeOptions>,
): T;

export function deepMerge<
  T extends Record<PropertyKey, unknown>,
  U extends Record<PropertyKey, unknown>,
  Options extends DeepMergeOptions,
>(
  record: Readonly<T>,
  other: Readonly<U>,
  options?: Readonly<Options>,
): DeepMerge<T, U, Options>;

export function deepMerge<
  T extends Record<PropertyKey, unknown>,
  U extends Record<PropertyKey, unknown>,
  Options extends DeepMergeOptions = {
    arrays: "merge";
    sets: "merge";
    maps: "merge";
  },
>(
  record: Readonly<T>,
  other: Readonly<U>,
  options?: Readonly<Options>,
): DeepMerge<T, U, Options> {
  return deepMergeInternal(record, other, new Set(), options);
}

function deepMergeInternal<
  T extends Record<PropertyKey, unknown>,
  U extends Record<PropertyKey, unknown>,
  Options extends DeepMergeOptions = {
    arrays: "merge";
    sets: "merge";
    maps: "merge";
  },
>(
  record: Readonly<T>,
  other: Readonly<U>,
  seen: Set<NonNullable<unknown>>,
  options?: Readonly<Options>,
) {
  // Extract options
  // Clone left operand to avoid performing mutations in-place
  type Result = DeepMerge<T, U, Options>;
  const result: Partial<Result> = {};

  const keys = new Set([
    ...getKeys(record),
    ...getKeys(other),
  ]) as Set<keyof Result>;

  // Iterate through each key of other object and use correct merging strategy
  for (const key of keys) {
    // Skip to prevent Object.prototype.__proto__ accessor property calls on non-Deno platforms
    if (key === "__proto__") {
      continue;
    }

    type ResultMember = Result[typeof key];

    const a = record[key] as ResultMember;

    if (!hasOwn(other, key)) {
      result[key] = a;

      continue;
    }

    const b = other[key] as ResultMember;

    if (
      isNonNullObject(a) && isNonNullObject(b) && !seen.has(a) && !seen.has(b)
    ) {
      seen.add(a);
      seen.add(b);
      result[key] = mergeObjects(a, b, seen, options) as ResultMember;

      continue;
    }

    // Override value
    result[key] = b;
  }

  return result as Result;
}

function mergeObjects(
  left: Readonly<NonNullable<Record<string, unknown>>>,
  right: Readonly<NonNullable<Record<string, unknown>>>,
  seen: Set<NonNullable<unknown>>,
  options: Readonly<DeepMergeOptions> = {
    arrays: "merge",
    sets: "merge",
    maps: "merge",
  },
): Readonly<NonNullable<Record<string, unknown> | Iterable<unknown>>> {
  // Recursively merge mergeable objects
  if (isMergeable(left) && isMergeable(right)) {
    return deepMergeInternal(left, right, seen, options);
  }

  if (isIterable(left) && isIterable(right)) {
    // Handle arrays
    if ((Array.isArray(left)) && (Array.isArray(right))) {
      if (options.arrays === "merge") {
        return left.concat(right);
      }

      return right;
    }

    // Handle maps
    if ((left instanceof Map) && (right instanceof Map)) {
      if (options.maps === "merge") {
        return new Map([
          ...left,
          ...right,
        ]);
      }

      return right;
    }

    // Handle sets
    if ((left instanceof Set) && (right instanceof Set)) {
      if (options.sets === "merge") {
        return new Set([
          ...left,
          ...right,
        ]);
      }

      return right;
    }
  }

  return right;
}

/**
 * Test whether a value is mergeable or not
 * Builtins that look like objects, null and user defined classes
 * are not considered mergeable (it means that reference will be copied)
 */
function isMergeable(
  value: NonNullable<unknown>,
): value is Record<PropertyKey, unknown> {
  return Object.getPrototypeOf(value) === Object.prototype;
}

function isIterable(
  value: NonNullable<unknown>,
): value is Iterable<unknown> {
  return typeof (value as Iterable<unknown>)[Symbol.iterator] === "function";
}

function isNonNullObject(
  value: unknown,
): value is NonNullable<Record<string, unknown>> {
  return value !== null && typeof value === "object";
}

function getKeys<T extends Record<string, unknown>>(record: T): Array<keyof T> {
  const ret = Object.getOwnPropertySymbols(record) as Array<keyof T>;
  filterInPlace(
    ret,
    (key) => Object.prototype.propertyIsEnumerable.call(record, key),
  );
  ret.push(...(Object.keys(record) as Array<keyof T>));

  return ret;
}

/** Merging strategy */
export type MergingStrategy = "replace" | "merge";

/** Deep merge options */
export type DeepMergeOptions = {
  /** Merging strategy for arrays */
  arrays?: MergingStrategy;
  /** Merging strategy for Maps */
  maps?: MergingStrategy;
  /** Merging strategy for Sets */
  sets?: MergingStrategy;
};

/**
 * How does recursive typing works ?
 *
 * Deep merging process is handled through `DeepMerge<T, U, Options>` type.
 * If both T and U are Records, we recursively merge them,
 * else we treat them as primitives.
 *
 * Merging process is handled through `Merge<T, U>` type, in which
 * we remove all maps, sets, arrays and records so we can handle them
 * separately depending on merging strategy:
 *
 *    Merge<
 *      {foo: string},
 *      {bar: string, baz: Set<unknown>},
 *    > // "foo" and "bar" will be handled with `MergeRightOmitComplexes`
 *      // "baz" will be handled with `MergeAll*` type
 *
 * `MergeRightOmitComplexes<T, U>` will do the above: all T's
 * exclusive keys will be kept, though common ones with U will have their
 * typing overridden instead:
 *
 *    MergeRightOmitComplexes<
 *      {foo: string, baz: number},
 *      {foo: boolean, bar: string}
 *    > // {baz: number, foo: boolean, bar: string}
 *      // "baz" was kept from T
 *      // "foo" was overridden by U's typing
 *      // "bar" was added from U
 *
 * For Maps, Arrays, Sets and Records, we use `MergeAll*<T, U>` utility
 * types. They will extract relevant data structure from both T and U
 * (providing that both have same data data structure, except for typing).
 *
 * From these, `*ValueType<T>` will extract values (and keys) types to be
 * able to create a new data structure with an union typing from both
 * data structure of T and U:
 *
 *    MergeAllSets<
 *      {foo: Set<number>},
 *      {foo: Set<string>}
 *    > // `SetValueType` will extract "number" for T
 *      // `SetValueType` will extract "string" for U
 *      // `MergeAllSets` will infer type as Set<number|string>
 *      // Process is similar for Maps, Arrays, and Sets
 *
 * `DeepMerge<T, U, Options>` is taking a third argument to be handle to
 * infer final typing depending on merging strategy:
 *
 *    & (Options extends { sets: "replace" } ? PartialByType<U, Set<unknown>>
 *      : MergeAllSets<T, U>)
 *
 * In the above line, if "Options" have its merging strategy for Sets set to
 * "replace", instead of performing merging of Sets type, it will take the
 * typing from right operand (U) instead, effectively replacing the typing.
 *
 * An additional note, we use `ExpandRecursively<T>` utility type to expand
 * the resulting typing and hide all the typing logic of deep merging so it is
 * more user friendly.
 */

/** Force intellisense to expand the typing to hide merging typings */
type ExpandRecursively<T> = T extends Record<PropertyKey, unknown>
  ? T extends infer O ? { [K in keyof O]: ExpandRecursively<O[K]> } : never
  : T;

/** Filter of keys matching a given type */
type PartialByType<T, U> = {
  [K in keyof T as T[K] extends U ? K : never]: T[K];
};

/** Get set values type */
type SetValueType<T> = T extends Set<infer V> ? V : never;

/** Merge all sets types definitions from keys present in both objects */
type MergeAllSets<
  T,
  U,
  X = PartialByType<T, Set<unknown>>,
  Y = PartialByType<U, Set<unknown>>,
  Z = {
    [K in keyof X & keyof Y]: Set<SetValueType<X[K]> | SetValueType<Y[K]>>;
  },
> = Z;

/** Get array values type */
type ArrayValueType<T> = T extends Array<infer V> ? V : never;

/** Merge all sets types definitions from keys present in both objects */
type MergeAllArrays<
  T,
  U,
  X = PartialByType<T, Array<unknown>>,
  Y = PartialByType<U, Array<unknown>>,
  Z = {
    [K in keyof X & keyof Y]: Array<
      ArrayValueType<X[K]> | ArrayValueType<Y[K]>
    >;
  },
> = Z;

/** Get map values types */
type MapKeyType<T> = T extends Map<infer K, unknown> ? K : never;

/** Get map values types */
type MapValueType<T> = T extends Map<unknown, infer V> ? V : never;

/** Merge all sets types definitions from keys present in both objects */
type MergeAllMaps<
  T,
  U,
  X = PartialByType<T, Map<unknown, unknown>>,
  Y = PartialByType<U, Map<unknown, unknown>>,
  Z = {
    [K in keyof X & keyof Y]: Map<
      MapKeyType<X[K]> | MapKeyType<Y[K]>,
      MapValueType<X[K]> | MapValueType<Y[K]>
    >;
  },
> = Z;

/** Merge all records types definitions from keys present in both objects */
type MergeAllRecords<
  T,
  U,
  Options,
  X = PartialByType<T, Record<PropertyKey, unknown>>,
  Y = PartialByType<U, Record<PropertyKey, unknown>>,
  Z = {
    [K in keyof X & keyof Y]: DeepMerge<X[K], Y[K], Options>;
  },
> = Z;

/** Exclude map, sets and array from type */
type OmitComplexes<T> = Omit<
  T,
  keyof PartialByType<
    T,
    | Map<unknown, unknown>
    | Set<unknown>
    | Array<unknown>
    | Record<PropertyKey, unknown>
  >
>;

/** Object with keys in either T or U but not in both */
type ObjectXorKeys<
  T,
  U,
  X = Omit<T, keyof U> & Omit<U, keyof T>,
  Y = { [K in keyof X]: X[K] },
> = Y;

/** Merge two objects, with left precedence */
type MergeRightOmitComplexes<
  T,
  U,
  X = ObjectXorKeys<T, U> & OmitComplexes<{ [K in keyof U]: U[K] }>,
> = X;

/** Merge two objects */
type Merge<
  T,
  U,
  Options,
  X =
    & MergeRightOmitComplexes<T, U>
    & MergeAllRecords<T, U, Options>
    & (Options extends { sets: "replace" } ? PartialByType<U, Set<unknown>>
      : MergeAllSets<T, U>)
    & (Options extends { arrays: "replace" } ? PartialByType<U, Array<unknown>>
      : MergeAllArrays<T, U>)
    & (Options extends { maps: "replace" }
      ? PartialByType<U, Map<unknown, unknown>>
      : MergeAllMaps<T, U>),
> = ExpandRecursively<X>;

/** Merge deeply two objects */
export type DeepMerge<
  T,
  U,
  Options = Record<string, MergingStrategy>,
> =
  // Handle objects
  [T, U] extends [Record<PropertyKey, unknown>, Record<PropertyKey, unknown>]
    ? Merge<T, U, Options>
    // Handle primitives
    : T | U;

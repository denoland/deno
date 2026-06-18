// Copyright 2018-2026 the Deno authors. MIT license.
// This module is browser compatible.

/** Default merging options - cached to avoid object allocation on each call */
const DEFAULT_OPTIONS: DeepMergeOptions = {
  arrays: "merge",
  sets: "merge",
  maps: "merge",
};

/**
 * Merges the two given records, recursively merging any nested records with the
 * second collection overriding the first in case of conflict.
 *
 * For arrays, maps and sets, a merging strategy can be specified to either
 * `replace` values, or `merge` them instead.
 *
 * @typeParam T Type of the first record
 *
 * @param record First record to merge.
 * @param other Second record to merge.
 * @param options Merging options.
 *
 * @returns A new record with the merged values.
 *
 * @example Merge objects
 * ```ts
 * import { deepMerge } from "@std/collections/deep-merge";
 * import { assertEquals } from "@std/assert";
 *
 * const a = { foo: true };
 * const b = { foo: { bar: true } };
 *
 * const result = deepMerge(a, b);
 *
 * const expected = { foo: { bar: true } };
 *
 * assertEquals(result, expected);
 * ```
 *
 * @example Merge arrays
 * ```ts
 * import { deepMerge } from "@std/collections/deep-merge";
 * import { assertEquals } from "@std/assert";
 *
 * const a = { foo: [1, 2] };
 * const b = { foo: [3, 4] };
 *
 * const result = deepMerge(a, b);
 *
 * const expected = { foo: [1, 2, 3, 4] };
 *
 * assertEquals(result, expected);
 * ```
 *
 * @example Merge maps
 * ```ts
 * import { deepMerge } from "@std/collections/deep-merge";
 * import { assertEquals } from "@std/assert";
 *
 * const a = { foo: new Map([["a", 1]]) };
 * const b = { foo: new Map([["b", 2]]) };
 *
 * const result = deepMerge(a, b);
 *
 * const expected = { foo: new Map([["a", 1], ["b", 2]]) };
 *
 * assertEquals(result, expected);
 * ```
 *
 * @example Merge sets
 * ```ts
 * import { deepMerge } from "@std/collections/deep-merge";
 * import { assertEquals } from "@std/assert";
 *
 * const a = { foo: new Set([1]) };
 * const b = { foo: new Set([2]) };
 *
 * const result = deepMerge(a, b);
 *
 * const expected = { foo: new Set([1, 2]) };
 *
 * assertEquals(result, expected);
 * ```
 *
 * @example Merge with custom options
 * ```ts
 * import { deepMerge } from "@std/collections/deep-merge";
 * import { assertEquals } from "@std/assert";
 *
 * const a = { foo: [1, 2] };
 * const b = { foo: [3, 4] };
 *
 * const result = deepMerge(a, b, { arrays: "replace" });
 *
 * const expected = { foo: [3, 4] };
 *
 * assertEquals(result, expected);
 * ```
 */
export function deepMerge<
  T extends Record<PropertyKey, unknown>,
>(
  record: Partial<Readonly<T>>,
  other: Partial<Readonly<T>>,
  options?: Readonly<DeepMergeOptions>,
): T;
/**
 * Merges the two given records, recursively merging any nested records with the
 * second collection overriding the first in case of conflict.
 *
 * For arrays, maps and sets, a merging strategy can be specified to either
 * `replace` values, or `merge` them instead.
 *
 * @typeParam T Type of the first record
 * @typeParam U Type of the second record
 * @typeParam Options Merging options
 *
 * @param record First record to merge.
 * @param other Second record to merge.
 * @param options Merging options.
 *
 * @returns A new record with the merged values.
 *
 * @example Merge objects
 * ```ts
 * import { deepMerge } from "@std/collections/deep-merge";
 * import { assertEquals } from "@std/assert";
 *
 * const a = { foo: true };
 * const b = { foo: { bar: true } };
 *
 * const result = deepMerge(a, b);
 *
 * const expected = { foo: { bar: true } };
 *
 * assertEquals(result, expected);
 * ```
 *
 * @example Merge arrays
 * ```ts
 * import { deepMerge } from "@std/collections/deep-merge";
 * import { assertEquals } from "@std/assert";
 *
 * const a = { foo: [1, 2] };
 * const b = { foo: [3, 4] };
 *
 * const result = deepMerge(a, b);
 *
 * const expected = { foo: [1, 2, 3, 4] };
 *
 * assertEquals(result, expected);
 * ```
 *
 * @example Merge maps
 * ```ts
 * import { deepMerge } from "@std/collections/deep-merge";
 * import { assertEquals } from "@std/assert";
 *
 * const a = { foo: new Map([["a", 1]]) };
 * const b = { foo: new Map([["b", 2]]) };
 *
 * const result = deepMerge(a, b);
 *
 * const expected = { foo: new Map([["a", 1], ["b", 2]]) };
 *
 * assertEquals(result, expected);
 * ```
 *
 * @example Merge sets
 * ```ts
 * import { deepMerge } from "@std/collections/deep-merge";
 * import { assertEquals } from "@std/assert";
 *
 * const a = { foo: new Set([1]) };
 * const b = { foo: new Set([2]) };
 *
 * const result = deepMerge(a, b);
 *
 * const expected = { foo: new Set([1, 2]) };
 *
 * assertEquals(result, expected);
 * ```
 *
 * @example Merge with custom options
 * ```ts
 * import { deepMerge } from "@std/collections/deep-merge";
 * import { assertEquals } from "@std/assert";
 *
 * const a = { foo: [1, 2] };
 * const b = { foo: [3, 4] };
 *
 * const result = deepMerge(a, b, { arrays: "replace" });
 *
 * const expected = { foo: [3, 4] };
 *
 * assertEquals(result, expected);
 * ```
 */
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
  return deepMergeInternal(
    record,
    other,
    new Set(),
    options ?? DEFAULT_OPTIONS as Options,
  );
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
  options: Readonly<Options>,
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

    if (!Object.hasOwn(other, key)) {
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
  options: Readonly<DeepMergeOptions>,
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
        const result = new Map(left);
        for (const [k, v] of right) {
          result.set(k, v);
        }
        return result;
      }

      return right;
    }

    // Handle sets
    if ((left instanceof Set) && (right instanceof Set)) {
      if (options.sets === "merge") {
        const result = new Set(left);
        for (const v of right) {
          result.add(v);
        }
        return result;
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
  const keys = Object.keys(record) as Array<keyof T>;
  const symbols = Object.getOwnPropertySymbols(record);

  // Fast path: most objects have no symbol keys
  if (symbols.length === 0) return keys;

  for (const sym of symbols) {
    if (Object.prototype.propertyIsEnumerable.call(record, sym)) {
      keys.push(sym as keyof T);
    }
  }

  return keys;
}

/** Merging strategy */
export type MergingStrategy = "replace" | "merge";

/** Options for {@linkcode deepMerge}. */
export type DeepMergeOptions = {
  /**
   * Merging strategy for arrays
   *
   * @default {"merge"}
   */
  arrays?: MergingStrategy;
  /**
   * Merging strategy for maps.
   *
   * @default {"merge"}
   */
  maps?: MergingStrategy;
  /**
   * Merging strategy for sets.
   *
   * @default {"merge"}
   */
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
export type ExpandRecursively<T> = T extends Record<PropertyKey, unknown>
  ? T extends infer O ? { [K in keyof O]: ExpandRecursively<O[K]> } : never
  : T;

/** Filter of keys matching a given type */
export type PartialByType<T, U> = {
  [K in keyof T as T[K] extends U ? K : never]: T[K];
};

/** Get set values type */
export type SetValueType<T> = T extends Set<infer V> ? V : never;

/** Merge all sets types definitions from keys present in both objects */
export type MergeAllSets<
  T,
  U,
  X = PartialByType<T, Set<unknown>>,
  Y = PartialByType<U, Set<unknown>>,
  Z = {
    [K in keyof X & keyof Y]: Set<SetValueType<X[K]> | SetValueType<Y[K]>>;
  },
> = Z;

/** Get array values type */
export type ArrayValueType<T> = T extends Array<infer V> ? V : never;

/** Merge all arrays types definitions from keys present in both objects */
export type MergeAllArrays<
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
export type MapKeyType<T> = T extends Map<infer K, unknown> ? K : never;

/** Get map values types */
export type MapValueType<T> = T extends Map<unknown, infer V> ? V : never;

/** Merge all maps types definitions from keys present in both objects */
export type MergeAllMaps<
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
export type MergeAllRecords<
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
export type OmitComplexes<T> = Omit<
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
export type ObjectXorKeys<
  T,
  U,
  X = Omit<T, keyof U> & Omit<U, keyof T>,
  Y = { [K in keyof X]: X[K] },
> = Y;

/** Merge two objects, with left precedence */
export type MergeRightOmitComplexes<
  T,
  U,
  X = ObjectXorKeys<T, U> & OmitComplexes<{ [K in keyof U]: U[K] }>,
> = X;

/** Merge two objects */
export type Merge<
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

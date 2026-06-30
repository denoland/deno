// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2026 the Deno authors. MIT license.
// This module is browser compatible.

import { DumperState } from "./_dumper_state.ts";
import { SCHEMA_MAP, type SchemaType } from "./_schema.ts";
import type { StyleVariant } from "./_type.ts";

export type { SchemaType, StyleVariant };

/** Options for {@linkcode stringify}. */
export type StringifyOptions = {
  /**
   * Indentation width to use (in spaces).
   *
   * @default {2}
   */
  indent?: number;
  /**
   * When true, adds an indentation level to array elements.
   *
   * @default {true}
   */
  arrayIndent?: boolean;
  /**
   * Do not throw on invalid types (like function in the safe schema) and skip
   * pairs and single values with such types.
   *
   * @default {false}
   */
  skipInvalid?: boolean;
  /**
   * Specifies level of nesting, when to switch from block to flow style for
   * collections. `-1` means block style everywhere.
   *
   * @default {-1}
   */
  flowLevel?: number;
  /** Each tag may have own set of styles.	- "tag" => "style" map. */
  styles?: Record<string, StyleVariant>;
  /**
   * Name of the schema to use.
   *
   * @default {"default"}
   */
  schema?: SchemaType;
  /**
   * If true, sort keys when dumping YAML in ascending, ASCII character order.
   * If a function, use the function to sort the keys.
   * If a function is specified, the function must return a negative value
   * if first argument is less than second argument, zero if they're equal
   * and a positive value otherwise.
   *
   * @default {false}
   */
  sortKeys?: boolean | ((a: string, b: string, depth: number) => number);
  /**
   * Set max line width.
   *
   * @default {80}
   */
  lineWidth?: number;
  /**
   * If false, don't convert duplicate objects into references.
   *
   * @default {true}
   */
  useAnchors?: boolean;
  /**
   * If false don't try to be compatible with older yaml versions.
   * Currently: don't quote "yes", "no" and so on,
   * as required for YAML 1.1.
   *
   * @default {true}
   */
  compatMode?: boolean;
  /**
   * If true flow sequences will be condensed, omitting the
   * space between `key: value` or `a, b`. Eg. `'[a,b]'` or `{a:{b:c}}`.
   * Can be useful when using yaml for pretty URL query params
   * as spaces are %-encoded.
   *
   * @default {false}
   */
  condenseFlow?: boolean;
};

/**
 * Converts a JavaScript object or value to a YAML document string.
 *
 * @example Usage
 * ```ts
 * import { stringify } from "@std/yaml/stringify";
 * import { assertEquals } from "@std/assert";
 *
 * const data = { id: 1, name: "Alice" };
 * const yaml = stringify(data);
 *
 * assertEquals(yaml, "id: 1\nname: Alice\n");
 * ```
 *
 * @throws {TypeError} If `data` contains invalid types.
 * @param data The data to serialize.
 * @param options The options for serialization.
 * @returns A YAML string.
 */
export function stringify(
  data: unknown,
  options: StringifyOptions = {},
): string {
  const state = new DumperState({
    ...options,
    schema: SCHEMA_MAP.get(options.schema!)!,
  });
  return state.stringify(data);
}

// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { CbFunction, load, loadAll } from "./_loader/loader.ts";
import type { LoaderStateOptions } from "./_loader/loader_state.ts";

export type ParseOptions = LoaderStateOptions;

/**
 * Parses `content` as single YAML document.
 *
 * Returns a JavaScript object or throws `YAMLError` on error.
 * By default, does not support regexps, functions and undefined. This method is safe for untrusted data.
 */
export function parse(content: string, options?: ParseOptions): unknown {
  return load(content, options);
}

/**
 * Same as `parse()`, but understands multi-document sources.
 * Applies iterator to each document if specified, or returns array of documents.
 *
 * @example
 * ```ts
 * import { parseAll } from "https://deno.land/std@$STD_VERSION/yaml/parse.ts";
 *
 * const data = parseAll(`
 * ---
 * id: 1
 * name: Alice
 * ---
 * id: 2
 * name: Bob
 * ---
 * id: 3
 * name: Eve
 * `);
 * console.log(data);
 * // => [ { id: 1, name: "Alice" }, { id: 2, name: "Bob" }, { id: 3, name: "Eve" } ]
 * ```
 */
export function parseAll(
  content: string,
  iterator: CbFunction,
  options?: ParseOptions,
): void;
export function parseAll(content: string, options?: ParseOptions): unknown;
export function parseAll(
  content: string,
  iterator?: CbFunction | ParseOptions,
  options?: ParseOptions,
): unknown {
  return loadAll(content, iterator, options);
}

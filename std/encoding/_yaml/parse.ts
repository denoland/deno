// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { CbFunction, load, loadAll } from "./loader/loader.ts";
import type { LoaderStateOptions } from "./loader/loader_state.ts";

export type ParseOptions = LoaderStateOptions;

/**
 * Parses `content` as single YAML document.
 *
 * Returns a JavaScript object or throws `YAMLException` on error.
 * By default, does not support regexps, functions and undefined. This method is safe for untrusted data.
 *
 */
export function parse(content: string, options?: ParseOptions): unknown {
  return load(content, options);
}

/**
 * Same as `parse()`, but understands multi-document sources.
 * Applies iterator to each document if specified, or returns array of documents.
 */
export function parseAll(
  content: string,
  iterator?: CbFunction,
  options?: ParseOptions,
): unknown {
  return loadAll(content, iterator, options);
}

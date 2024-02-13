// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { extensionsByType } from "./extensions_by_type.ts";

/**
 * For a given media type, return the most relevant extension, or `undefined`
 * if no extension can be found.
 *
 * Extensions are returned without a leading `.`.
 *
 * @example
 * ```ts
 * import { extension } from "https://deno.land/std@$STD_VERSION/media_types/extension.ts";
 *
 * extension("text/plain"); // `txt`
 * extension("application/json"); // `json`
 * extension("text/html; charset=UTF-8"); // `html`
 * extension("application/foo"); // undefined
 * ```
 */
export function extension(type: string): string | undefined {
  const exts = extensionsByType(type);
  if (exts) {
    return exts[0];
  }
  return undefined;
}

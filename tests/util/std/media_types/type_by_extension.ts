// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { types } from "./_db.ts";

/**
 * @deprecated (will be removed in 0.209.0) Use {@linkcode import("./content_type.ts").contentType} instead.
 *
 * Returns the media type associated with the file extension. Values are
 * normalized to lower case and matched irrespective of a leading `.`.
 *
 * When `extension` has no associated type, the function returns `undefined`.
 *
 * @example
 * ```ts
 * import { typeByExtension } from "https://deno.land/std@$STD_VERSION/media_types/type_by_extension.ts";
 *
 * typeByExtension("js"); // `application/json`
 * typeByExtension(".HTML"); // `text/html`
 * typeByExtension("foo"); // undefined
 * typeByExtension("file.json"); // undefined
 * ```
 */
export function typeByExtension(extension: string): string | undefined {
  extension = extension.startsWith(".") ? extension.slice(1) : extension;
  // @ts-ignore workaround around denoland/dnt#148
  return types.get(extension.toLowerCase());
}

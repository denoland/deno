// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

/** Provides functions for dealing with and matching ETags, including
 * {@linkcode calculate} to calculate an etag for a given entity,
 * {@linkcode ifMatch} for validating if an ETag matches against a `If-Match`
 * header and {@linkcode ifNoneMatch} for validating an Etag against an
 * `If-None-Match` header.
 *
 * See further information on the `ETag` header on
 * [MDN](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/ETag).
 *
 * @module
 */

import { encodeBase64 as base64Encode } from "../encoding/base64.ts";

/** Just the part of `Deno.FileInfo` that is required to calculate an `ETag`,
 * so partial or user generated file information can be passed. */
export interface FileInfo {
  mtime: Date | null;
  size: number;
}

type Entity = string | Uint8Array | FileInfo;

const encoder = new TextEncoder();

const DEFAULT_ALGORITHM: AlgorithmIdentifier = "SHA-256";

export interface ETagOptions {
  /** A digest algorithm to use to calculate the etag. Defaults to
   * `"FNV32A"`. */
  algorithm?: AlgorithmIdentifier;

  /** Override the default behavior of calculating the `ETag`, either forcing
   * a tag to be labelled weak or not. */
  weak?: boolean;
}

function isFileInfo(value: unknown): value is FileInfo {
  return Boolean(
    value && typeof value === "object" && "mtime" in value && "size" in value,
  );
}

async function calcEntity(
  entity: string | Uint8Array,
  { algorithm = DEFAULT_ALGORITHM }: ETagOptions,
) {
  // a short circuit for zero length entities
  if (entity.length === 0) {
    return `0-47DEQpj8HBSa+/TImW+5JCeuQeR`;
  }

  if (typeof entity === "string") {
    entity = encoder.encode(entity);
  }

  const hash = base64Encode(await crypto.subtle.digest(algorithm, entity))
    .substring(0, 27);

  return `${entity.length.toString(16)}-${hash}`;
}

async function calcFileInfo(
  fileInfo: FileInfo,
  { algorithm = DEFAULT_ALGORITHM }: ETagOptions,
) {
  if (fileInfo.mtime) {
    const hash = base64Encode(
      await crypto.subtle.digest(
        algorithm,
        encoder.encode(fileInfo.mtime.toJSON()),
      ),
    ).substring(0, 27);
    return `${fileInfo.size.toString(16)}-${hash}`;
  }
}

/** Calculate an ETag for an entity. When the entity is a specific set of data
 * it will be fingerprinted as a "strong" tag, otherwise if it is just file
 * information, it will be calculated as a weak tag.
 *
 * ```ts
 * import { calculate } from "https://deno.land/std@$STD_VERSION/http/etag.ts";
 * import { assert } from "https://deno.land/std@$STD_VERSION/assert/assert.ts"
 *
 * const body = "hello deno!";
 *
 * const etag = await calculate(body);
 * assert(etag);
 *
 * const res = new Response(body, { headers: { etag } });
 * ```
 */
export async function calculate(
  entity: Entity,
  options: ETagOptions = {},
): Promise<string | undefined> {
  const weak = options.weak ?? isFileInfo(entity);
  const tag =
    await (isFileInfo(entity)
      ? calcFileInfo(entity, options)
      : calcEntity(entity, options));

  return tag ? weak ? `W/"${tag}"` : `"${tag}"` : undefined;
}

/** A helper function that takes the value from the `If-Match` header and a
 * calculated etag for the target. By using strong comparison, return `true` if
 * the values match, otherwise `false`.
 *
 * See MDN's [`If-Match`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/If-Match)
 * article for more information on how to use this function.
 *
 * ```ts
 * import {
 *   calculate,
 *   ifMatch,
 * } from "https://deno.land/std@$STD_VERSION/http/etag.ts";
 * import { assert } from "https://deno.land/std@$STD_VERSION/assert/assert.ts"
 *
 * const body = "hello deno!";
 *
 * Deno.serve(async (req) => {
 *   const ifMatchValue = req.headers.get("if-match");
 *   const etag = await calculate(body);
 *   assert(etag);
 *   if (!ifMatchValue || ifMatch(ifMatchValue, etag)) {
 *     return new Response(body, { status: 200, headers: { etag } });
 *   } else {
 *     return new Response(null, { status: 412, statusText: "Precondition Failed"});
 *   }
 * });
 * ```
 */
export function ifMatch(
  value: string | null,
  etag: string | undefined,
): boolean {
  // Weak tags cannot be matched and return false.
  if (!value || !etag || etag.startsWith("W/")) {
    return false;
  }
  if (value.trim() === "*") {
    return true;
  }
  const tags = value.split(/\s*,\s*/);
  return tags.includes(etag);
}

/** A helper function that takes the value from the `If-None-Match` header and
 * a calculated etag for the target entity and returns `false` if the etag for
 * the entity matches the supplied value, otherwise `true`.
 *
 * See MDN's [`If-None-Match`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/If-None-Match)
 * article for more information on how to use this function.
 *
 * ```ts
 * import {
 *   calculate,
 *   ifNoneMatch,
 * } from "https://deno.land/std@$STD_VERSION/http/etag.ts";
 * import { assert } from "https://deno.land/std@$STD_VERSION/assert/assert.ts"
 *
 * const body = "hello deno!";
 *
 * Deno.serve(async (req) => {
 *   const ifNoneMatchValue = req.headers.get("if-none-match");
 *   const etag = await calculate(body);
 *   assert(etag);
 *   if (!ifNoneMatch(ifNoneMatchValue, etag)) {
 *     return new Response(null, { status: 304, headers: { etag } });
 *   } else {
 *     return new Response(body, { status: 200, headers: { etag } });
 *   }
 * });
 * ```
 */
export function ifNoneMatch(
  value: string | null,
  etag: string | undefined,
): boolean {
  if (!value || !etag) {
    return true;
  }
  if (value.trim() === "*") {
    return false;
  }
  etag = etag.startsWith("W/") ? etag.slice(2) : etag;
  const tags = value.split(/\s*,\s*/).map((tag) =>
    tag.startsWith("W/") ? tag.slice(2) : tag
  );
  return !tags.includes(etag);
}

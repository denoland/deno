// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { bytesToUuid, uuidToBytes } from "./_common.ts";
import { concat } from "../bytes/concat.ts";
import { assert } from "../assert/assert.ts";
import { crypto } from "../crypto/crypto.ts";

const UUID_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[3][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

/**
 * Validate that the passed UUID is an RFC4122 v3 UUID.
 *
 * @example
 * ```ts
 * import { generate as generateV3, validate } from "https://deno.land/std@$STD_VERSION/uuid/v3.ts";
 *
 * validate(await generateV3("6ba7b811-9dad-11d1-80b4-00c04fd430c8", new Uint8Array())); // true
 * validate(crypto.randomUUID()); // false
 * validate("this-is-not-a-uuid"); // false
 * ```
 */
export function validate(id: string): boolean {
  return UUID_RE.test(id);
}

/**
 * Generate a RFC4122 v3 UUID (MD5 namespace).
 *
 * @example
 * ```js
 * import { generate } from "https://deno.land/std@$STD_VERSION/uuid/v3.ts";
 *
 * const NAMESPACE_URL = "6ba7b811-9dad-11d1-80b4-00c04fd430c8";
 *
 * const uuid = await generate(NAMESPACE_URL, new TextEncoder().encode("python.org"));
 * uuid === "22fe6191-c161-3d86-a432-a81f343eda08" // true
 * ```
 *
 * @param namespace The namespace to use, encoded as a UUID.
 * @param data The data to hash to calculate the MD5 digest for the UUID.
 */
export async function generate(
  namespace: string,
  data: Uint8Array,
): Promise<string> {
  // TODO(lino-levan): validate that `namespace` is a valid UUID.

  const space = uuidToBytes(namespace);
  assert(space.length === 16, "namespace must be a valid UUID");

  const toHash = concat([new Uint8Array(space), data]);
  const buffer = await crypto.subtle.digest("MD5", toHash);
  const bytes = new Uint8Array(buffer);

  bytes[6] = (bytes[6] & 0x0f) | 0x30;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;

  return bytesToUuid(bytes);
}

// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

const UUID_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

/**
 * Validate that the passed UUID is an RFC4122 v4 UUID.
 *
 * @example
 * ```ts
 * import { validate } from "https://deno.land/std@$STD_VERSION/uuid/v4.ts";
 * import { generate as generateV1 } from "https://deno.land/std@$STD_VERSION/uuid/v1.ts";
 *
 * validate(crypto.randomUUID()); // true
 * validate(generateV1() as string); // false
 * validate("this-is-not-a-uuid"); // false
 * ```
 */
export function validate(id: string): boolean {
  return UUID_RE.test(id);
}

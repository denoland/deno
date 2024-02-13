// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Generators and validators for UUIDs for versions v1, v3, v4 and v5.
 *
 * Consider using the web platform
 * [`crypto.randomUUID`](https://developer.mozilla.org/en-US/docs/Web/API/Crypto/randomUUID)
 * for v4 UUIDs instead.
 *
 * Based on https://github.com/kelektiv/node-uuid -> https://www.ietf.org/rfc/rfc4122.txt
 *
 * Support for RFC4122 version 1, 3, 4, and 5 UUIDs
 *
 * This module is browser compatible.
 *
 * @module
 */

export * from "./constants.ts";

import * as v1 from "./v1.ts";
import * as v3 from "./v3.ts";
import * as v4 from "./v4.ts";
import * as v5 from "./v5.ts";

export const NIL_UUID = "00000000-0000-0000-0000-000000000000";

/**
 * Check if the passed UUID is the nil UUID.
 *
 * ```js
 * import { isNil } from "https://deno.land/std@$STD_VERSION/uuid/mod.ts";
 *
 * isNil("00000000-0000-0000-0000-000000000000") // true
 * isNil(crypto.randomUUID()) // false
 * ```
 */
export function isNil(id: string): boolean {
  return id === NIL_UUID;
}

/**
 * Test a string to see if it is a valid UUID.
 *
 * ```js
 * import { validate } from "https://deno.land/std@$STD_VERSION/uuid/mod.ts"
 *
 * validate("not a UUID") // false
 * validate("6ec0bd7f-11c0-43da-975e-2a8ad9ebae0b") // true
 * ```
 */
export function validate(uuid: string): boolean {
  return /^(?:[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}|00000000-0000-0000-0000-000000000000)$/i
    .test(
      uuid,
    );
}

/**
 * Detect RFC version of a UUID.
 *
 * ```js
 * import { version } from "https://deno.land/std@$STD_VERSION/uuid/mod.ts"
 *
 * version("d9428888-122b-11e1-b85c-61cd3cbb3210") // 1
 * version("109156be-c4fb-41ea-b1b4-efe1671c5836") // 4
 * ```
 */
export function version(uuid: string): number {
  if (!validate(uuid)) {
    throw new TypeError("Invalid UUID");
  }

  return parseInt(uuid[14], 16);
}

export { v1, v3, v4, v5 };

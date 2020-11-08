// Based on https://github.com/kelektiv/node-uuid -> https://www.ietf.org/rfc/rfc4122.txt
// Supporting Support for RFC4122 version 1, 4, and 5 UUIDs
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as v1 from "./v1.ts";
import * as v4 from "./v4.ts";
import * as v5 from "./v5.ts";

export const NIL_UUID = "00000000-0000-0000-0000-000000000000";

/**
 * Checks if UUID is nil
 * @param val UUID value
 */
export function isNil(val: string): boolean {
  return val === NIL_UUID;
}

export { v1, v4, v5 };

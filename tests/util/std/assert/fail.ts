// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assert } from "./assert.ts";

/**
 * Forcefully throws a failed assertion
 */
export function fail(msg?: string): never {
  const msgSuffix = msg ? `: ${msg}` : ".";
  assert(false, `Failed assertion${msgSuffix}`);
}

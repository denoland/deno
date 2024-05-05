// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

/** A library of assertion functions.
 * If the assertion is false an `AssertionError` will be thrown which will
 * result in pretty-printed diff of failing assertion.
 *
 * This module is browser compatible, but do not rely on good formatting of
 * values for AssertionError messages in browsers.
 *
 * ```ts
 * import { assert } from "@std/assert/assert";
 *
 * assert("I am truthy"); // Doesn't throw
 * assert(false); // Throws `AssertionError`
 * ```
 *
 * @module
 */

export * from "./assert_equals.ts";
export * from "./assert.ts";
export * from "./fail.ts";

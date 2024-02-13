// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Extensions to the
 * [Web Crypto](https://developer.mozilla.org/en-US/docs/Web/API/Web_Crypto_API)
 * supporting additional encryption APIs, but also delegating to the built-in
 * APIs when possible.
 *
 * @module
 */

export * from "./crypto.ts";
export * from "./unstable_keystack.ts";
export * from "./timing_safe_equal.ts";
export * from "./to_hash_string.ts";

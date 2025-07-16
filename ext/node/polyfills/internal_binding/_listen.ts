// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
const { MathClz32 } = primordials;

/**
 * @param n Number to act on.
 * @return The number rounded up to the nearest power of 2.
 */
export function ceilPowOf2(n: number) {
  const roundPowOf2 = 1 << (31 - MathClz32(n));

  return roundPowOf2 < n ? roundPowOf2 * 2 : roundPowOf2;
}

/** Initial backoff delay of 5ms following a temporary accept failure. */
export const INITIAL_ACCEPT_BACKOFF_DELAY = 5;

/** Max backoff delay of 1s following a temporary accept failure. */
export const MAX_ACCEPT_BACKOFF_DELAY = 1000;

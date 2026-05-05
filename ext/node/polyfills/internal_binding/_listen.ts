// Copyright 2018-2026 the Deno authors. MIT license.
(function () {
const { primordials } = globalThis.__bootstrap;
const { MathClz32 } = primordials;

/**
 * @param n Number to act on.
 * @return The number rounded up to the nearest power of 2.
 */
function ceilPowOf2(n: number) {
  const roundPowOf2 = 1 << (31 - MathClz32(n));

  return roundPowOf2 < n ? roundPowOf2 * 2 : roundPowOf2;
}

/** Initial backoff delay of 5ms following a temporary accept failure. */
const INITIAL_ACCEPT_BACKOFF_DELAY = 5;

/** Max backoff delay of 1s following a temporary accept failure. */
const MAX_ACCEPT_BACKOFF_DELAY = 1000;

return {
  ceilPowOf2,
  INITIAL_ACCEPT_BACKOFF_DELAY,
  MAX_ACCEPT_BACKOFF_DELAY,
};
})();

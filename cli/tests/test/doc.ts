/**
 * ```js
 * import { ok } from "./doc.ts";
 *
 * ok();
 * ```
 *
 * ```ts
 * import { ok } from "./doc.ts";
 *
 * ok();
 * ```
 *
 */
export function ok() {
  // no-op
}

/**
 * ```js
 * import { fail } from "./doc.ts";
 *
 * fail();
 * ```
 *
 * ```ts
 * import { fail } from "./doc.ts";
 *
 * fail();
 * ```
 */
export function fail() {
  throw new Error();
}

/**
 * ```js
 * # import { hidden } from "./doc.ts";
 *
 * hidden();
 * ```
 *
 * ```ts
 * # import { hidden } from "./doc.ts";
 *
 * hidden();
 * ```
 */
export function hidden() {
  // no-op
}


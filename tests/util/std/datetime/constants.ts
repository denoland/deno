// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * The number of milliseconds in a second.
 *
 * @example
 * ```ts
 * import { SECOND } from "https://deno.land/std@$STD_VERSION/datetime/constants.ts";
 *
 * console.log(SECOND); // => 1000
 * ```
 */
export const SECOND = 1e3;
/**
 * The number of milliseconds in a minute.
 *
 * @example
 * ```ts
 * import { MINUTE } from "https://deno.land/std@$STD_VERSION/datetime/constants.ts";
 *
 * console.log(MINUTE); // => 60000 (60 * 1000)
 * ```
 */
export const MINUTE = SECOND * 60;
/**
 * The number of milliseconds in an hour.
 *
 * @example
 * ```ts
 * import { HOUR } from "https://deno.land/std@$STD_VERSION/datetime/constants.ts";
 *
 * console.log(HOUR); // => 3600000 (60 * 60 * 1000)
 * ```
 */
export const HOUR = MINUTE * 60;
/**
 * The number of milliseconds in a day.
 *
 * @example
 * ```ts
 * import { DAY } from "https://deno.land/std@$STD_VERSION/datetime/constants.ts";
 *
 * console.log(DAY); // => 86400000 (24 * 60 * 60 * 1000)
 * ```
 */
export const DAY = HOUR * 24;
/**
 * The number of milliseconds in a week.
 *
 * @example
 * ```ts
 * import { WEEK } from "https://deno.land/std@$STD_VERSION/datetime/constants.ts";
 *
 * console.log(WEEK); // => 604800000 (7 * 24 * 60 * 60 * 1000)
 * ```
 */
export const WEEK = DAY * 7;

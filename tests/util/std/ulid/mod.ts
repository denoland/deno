// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2023 Yoshiya Hinosawa. All rights reserved. MIT license.
// Copyright 2017 Alizain Feerasta. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * @module
 * @example
 * ```ts
 * import { ulid } from "https://deno.land/std@$STD_VERSION/ulid/mod.ts";
 * ulid(); // 01ARZ3NDEKTSV4RRFFQ69G5FAV
 * ```
 */

import {
  encodeRandom,
  encodeTime,
  ENCODING,
  ENCODING_LEN,
  monotonicFactory,
  RANDOM_LEN,
  TIME_LEN,
  TIME_MAX,
} from "./_util.ts";

/**
 * Extracts the timestamp given a ULID
 */
export function decodeTime(id: string): number {
  if (id.length !== TIME_LEN + RANDOM_LEN) {
    throw new Error("malformed ulid");
  }
  const time = id
    .substring(0, TIME_LEN)
    .split("")
    .reverse()
    .reduce((carry, char, index) => {
      const encodingIndex = ENCODING.indexOf(char);
      if (encodingIndex === -1) {
        throw new Error("invalid character found: " + char);
      }
      return (carry += encodingIndex * Math.pow(ENCODING_LEN, index));
    }, 0);
  if (time > TIME_MAX) {
    throw new Error("malformed ulid, timestamp too large");
  }
  return time;
}

/**
 * @example
 * ```ts
 * import { monotonicUlid } from "https://deno.land/std@$STD_VERSION/ulid/mod.ts";
 *
 * // Strict ordering for the same timestamp, by incrementing the least-significant random bit by 1
 * monotonicUlid(150000); // 000XAL6S41ACTAV9WEVGEMMVR8
 * monotonicUlid(150000); // 000XAL6S41ACTAV9WEVGEMMVR9
 * monotonicUlid(150000); // 000XAL6S41ACTAV9WEVGEMMVRA
 * monotonicUlid(150000); // 000XAL6S41ACTAV9WEVGEMMVRB
 * monotonicUlid(150000); // 000XAL6S41ACTAV9WEVGEMMVRC
 *
 * // Even if a lower timestamp is passed (or generated), it will preserve sort order
 * monotonicUlid(100000); // 000XAL6S41ACTAV9WEVGEMMVRD
 * ```
 */
export const monotonicUlid = monotonicFactory();

/**
 * @example
 * ```ts
 * import { ulid } from "https://deno.land/std@$STD_VERSION/ulid/mod.ts";
 * ulid(); // 01ARZ3NDEKTSV4RRFFQ69G5FAV
 *
 * // You can also input a seed time which will consistently give you the same string for the time component
 * ulid(1469918176385); // 01ARYZ6S41TSV4RRFFQ69G5FAV
 * ```
 */
export function ulid(seedTime: number = Date.now()): string {
  return encodeTime(seedTime, TIME_LEN) + encodeRandom(RANDOM_LEN);
}

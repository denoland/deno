// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

import { notImplemented } from "ext:deno_node/_utils.ts";
import randomBytes from "ext:deno_node/internal/crypto/_randomBytes.ts";
import randomFill, {
  randomFillSync,
} from "ext:deno_node/internal/crypto/_randomFill.ts";
import randomInt from "ext:deno_node/internal/crypto/_randomInt.ts";
import {
  validateFunction,
  validateInt32,
  validateObject,
} from "ext:deno_node/internal/validators.mjs";
import {
  isAnyArrayBuffer,
  isArrayBufferView,
} from "ext:deno_node/internal/util/types.ts";
import { ERR_INVALID_ARG_TYPE } from "ext:deno_node/internal/errors.ts";

export { default as randomBytes } from "ext:deno_node/internal/crypto/_randomBytes.ts";
export {
  default as randomFill,
  randomFillSync,
} from "ext:deno_node/internal/crypto/_randomFill.ts";
export { default as randomInt } from "ext:deno_node/internal/crypto/_randomInt.ts";

const { core } = globalThis.__bootstrap;
const { ops } = core;

export type LargeNumberLike =
  | ArrayBufferView
  | SharedArrayBuffer
  | ArrayBuffer
  | bigint;

export interface CheckPrimeOptions {
  /**
   * The number of Miller-Rabin probabilistic primality iterations to perform.
   * When the value is 0 (zero), a number of checks is used that yields a false positive rate of at most 2-64 for random input.
   * Care must be used when selecting a number of checks.
   * Refer to the OpenSSL documentation for the BN_is_prime_ex function nchecks options for more details.
   *
   * @default 0
   */
  checks?: number | undefined;
}

export function checkPrime(
  candidate: LargeNumberLike,
  callback: (err: Error | null, result: boolean) => void,
): void;
export function checkPrime(
  candidate: LargeNumberLike,
  options: CheckPrimeOptions,
  callback: (err: Error | null, result: boolean) => void,
): void;
export function checkPrime(
  candidate: LargeNumberLike,
  options: CheckPrimeOptions | ((err: Error | null, result: boolean) => void) =
    {},
  callback?: (err: Error | null, result: boolean) => void,
) {
  if (typeof options === "function") {
    callback = options;
    options = {};
  }

  validateFunction(callback, "callback");
  validateObject(options, "options");

  const {
    checks = 0,
  } = options!;

  validateInt32(checks, "options.checks", 0);

  let op = "op_node_check_prime_bytes_async";
  if (typeof candidate === "bigint") {
    op = "op_node_check_prime_async";
  } else if (!isAnyArrayBuffer(candidate) && !isArrayBufferView(candidate)) {
    throw new ERR_INVALID_ARG_TYPE(
      "candidate",
      [
        "ArrayBuffer",
        "TypedArray",
        "Buffer",
        "DataView",
        "bigint",
      ],
      candidate,
    );
  }

  core.opAsync2(op, candidate, checks).then(
    (result) => {
      callback?.(null, result);
    },
  ).catch((err) => {
    callback?.(err, false);
  });
}

export function checkPrimeSync(
  candidate: LargeNumberLike,
  options: CheckPrimeOptions = {},
): boolean {
  validateObject(options, "options");

  const {
    checks = 0,
  } = options!;

  validateInt32(checks, "options.checks", 0);

  if (typeof candidate === "bigint") {
    return ops.op_node_check_prime(candidate, checks);
  } else if (!isAnyArrayBuffer(candidate) && !isArrayBufferView(candidate)) {
    throw new ERR_INVALID_ARG_TYPE(
      "candidate",
      [
        "ArrayBuffer",
        "TypedArray",
        "Buffer",
        "DataView",
        "bigint",
      ],
      candidate,
    );
  }

  return ops.op_node_check_prime_bytes(candidate, checks);
}

export interface GeneratePrimeOptions {
  add?: LargeNumberLike | undefined;
  rem?: LargeNumberLike | undefined;
  /**
   * @default false
   */
  safe?: boolean | undefined;
  bigint?: boolean | undefined;
}

export interface GeneratePrimeOptionsBigInt extends GeneratePrimeOptions {
  bigint: true;
}

export interface GeneratePrimeOptionsArrayBuffer extends GeneratePrimeOptions {
  bigint?: false | undefined;
}

export function generatePrime(
  size: number,
  callback: (err: Error | null, prime: ArrayBuffer) => void,
): void;
export function generatePrime(
  size: number,
  options: GeneratePrimeOptionsBigInt,
  callback: (err: Error | null, prime: bigint) => void,
): void;
export function generatePrime(
  size: number,
  options: GeneratePrimeOptionsArrayBuffer,
  callback: (err: Error | null, prime: ArrayBuffer) => void,
): void;
export function generatePrime(
  size: number,
  options: GeneratePrimeOptions,
  callback: (err: Error | null, prime: ArrayBuffer | bigint) => void,
): void;
export function generatePrime(
  _size: number,
  _options?: unknown,
  _callback?: unknown,
) {
  notImplemented("crypto.generatePrime");
}

export function generatePrimeSync(size: number): ArrayBuffer;
export function generatePrimeSync(
  size: number,
  options: GeneratePrimeOptionsBigInt,
): bigint;
export function generatePrimeSync(
  size: number,
  options: GeneratePrimeOptionsArrayBuffer,
): ArrayBuffer;
export function generatePrimeSync(
  size: number,
  options: GeneratePrimeOptions,
): ArrayBuffer | bigint;
export function generatePrimeSync(
  _size: number,
  _options?:
    | GeneratePrimeOptionsBigInt
    | GeneratePrimeOptionsArrayBuffer
    | GeneratePrimeOptions,
): ArrayBuffer | bigint {
  notImplemented("crypto.generatePrimeSync");
}

export const randomUUID = () => globalThis.crypto.randomUUID();

export default {
  checkPrime,
  checkPrimeSync,
  generatePrime,
  generatePrimeSync,
  randomUUID,
  randomInt,
  randomBytes,
  randomFill,
  randomFillSync,
};

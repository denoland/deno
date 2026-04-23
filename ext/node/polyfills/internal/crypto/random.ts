// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import {
  op_node_check_prime_bytes,
  op_node_check_prime_bytes_async,
  op_node_gen_prime,
  op_node_gen_prime_async,
} from "ext:core/ops";

import randomBytes from "ext:deno_node/internal/crypto/_randomBytes.ts";
import randomFill, {
  randomFillSync,
} from "ext:deno_node/internal/crypto/_randomFill.mjs";
import randomInt from "ext:deno_node/internal/crypto/_randomInt.ts";
import {
  validateBoolean,
  validateFunction,
  validateInt32,
  validateObject,
} from "ext:deno_node/internal/validators.mjs";
import {
  isAnyArrayBuffer,
  isArrayBufferView,
} from "ext:deno_node/internal/util/types.ts";
import {
  ERR_INVALID_ARG_TYPE,
  ERR_OUT_OF_RANGE,
  NodeError,
  NodeRangeError,
} from "ext:deno_node/internal/errors.ts";
import { Buffer } from "node:buffer";

export { default as randomBytes } from "ext:deno_node/internal/crypto/_randomBytes.ts";
export {
  default as randomFill,
  randomFillSync,
} from "ext:deno_node/internal/crypto/_randomFill.mjs";
export { default as randomInt } from "ext:deno_node/internal/crypto/_randomInt.ts";

// OpenSSL BIGNUM max size: INT_MAX / (4 * BN_BITS2) words * 8 bytes/word
// On 64-bit: (2^31 - 1) / 256 * 8 = 67108856 bytes
const OPENSSL_BIGNUM_MAX_BYTES = (((2 ** 31 - 1) / (4 * 64)) | 0) * 8;

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

  let candidateBytes: ArrayBufferView | ArrayBuffer;
  if (typeof candidate === "bigint") {
    if (candidate < 0) {
      throw new ERR_OUT_OF_RANGE("candidate", ">= 0", candidate);
    }
    candidateBytes = bigintToBytes(candidate);
  } else if (isAnyArrayBuffer(candidate) || isArrayBufferView(candidate)) {
    const byteLength = isArrayBufferView(candidate)
      ? (candidate as ArrayBufferView).byteLength
      : (candidate as ArrayBuffer).byteLength;
    if (byteLength > OPENSSL_BIGNUM_MAX_BYTES) {
      throw new NodeError(
        "ERR_OSSL_BN_BIGNUM_TOO_LONG",
        "bignum too long",
      );
    }
    candidateBytes = candidate;
  } else {
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

  op_node_check_prime_bytes_async(candidateBytes, checks).then(
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

  let candidateBytes: ArrayBufferView | ArrayBuffer;
  if (typeof candidate === "bigint") {
    if (candidate < 0) {
      throw new ERR_OUT_OF_RANGE("candidate", ">= 0", candidate);
    }
    candidateBytes = bigintToBytes(candidate);
  } else if (isAnyArrayBuffer(candidate) || isArrayBufferView(candidate)) {
    const byteLength = isArrayBufferView(candidate)
      ? (candidate as ArrayBufferView).byteLength
      : (candidate as ArrayBuffer).byteLength;
    if (byteLength > OPENSSL_BIGNUM_MAX_BYTES) {
      throw new NodeError(
        "ERR_OSSL_BN_BIGNUM_TOO_LONG",
        "bignum too long",
      );
    }
    candidateBytes = candidate;
  } else {
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

  return op_node_check_prime_bytes(candidateBytes, checks);
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

export function generatePrime(
  size: number,
  options: GeneratePrimeOptions = {},
  callback?: (err: Error | null, prime: ArrayBuffer | bigint) => void,
) {
  validateInt32(size, "size", 1);
  if (typeof options === "function") {
    callback = options;
    options = {};
  }
  validateFunction(callback, "callback");
  const {
    bigint,
    safe,
    add,
    rem,
  } = validateRandomPrimeJob(size, options);
  op_node_gen_prime_async(size, safe, add ?? null, rem ?? null).then(
    (prime: Uint8Array) => {
      const result = bigint
        ? arrayBufferToUnsignedBigInt(prime.buffer)
        : prime.buffer;
      callback?.(null, result);
    },
    (err: Error) => {
      callback?.(err, null as unknown as ArrayBuffer);
    },
  );
}

export function generatePrimeSync(
  size: number,
  options: GeneratePrimeOptions = {},
): ArrayBuffer | bigint {
  const {
    bigint,
    safe,
    add,
    rem,
  } = validateRandomPrimeJob(size, options);

  const prime = op_node_gen_prime(size, safe, add ?? null, rem ?? null);
  if (bigint) return arrayBufferToUnsignedBigInt(prime.buffer);
  return prime.buffer;
}

interface ValidatedPrimeOptions {
  safe: boolean;
  bigint: boolean;
  add?: Uint8Array;
  rem?: Uint8Array;
}

function toUint8Array(
  value: ArrayBuffer | ArrayBufferView | Buffer,
): Uint8Array {
  if (value instanceof Uint8Array) {
    return value;
  }
  if (isArrayBufferView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  }
  return new Uint8Array(value);
}

function validateRandomPrimeJob(
  size: number,
  options: GeneratePrimeOptions,
): ValidatedPrimeOptions {
  validateInt32(size, "size", 1);
  validateObject(options, "options");

  let {
    safe = false,
    bigint = false,
    add,
    rem,
  } = options!;

  validateBoolean(safe, "options.safe");
  validateBoolean(bigint, "options.bigint");

  if (add !== undefined) {
    if (typeof add === "bigint") {
      add = unsignedBigIntToBuffer(add, "options.add");
    } else if (!isAnyArrayBuffer(add) && !isArrayBufferView(add)) {
      throw new ERR_INVALID_ARG_TYPE(
        "options.add",
        [
          "ArrayBuffer",
          "TypedArray",
          "Buffer",
          "DataView",
          "bigint",
        ],
        add,
      );
    }
  }

  if (rem !== undefined) {
    if (typeof rem === "bigint") {
      rem = unsignedBigIntToBuffer(rem, "options.rem");
    } else if (!isAnyArrayBuffer(rem) && !isArrayBufferView(rem)) {
      throw new ERR_INVALID_ARG_TYPE(
        "options.rem",
        [
          "ArrayBuffer",
          "TypedArray",
          "Buffer",
          "DataView",
          "bigint",
        ],
        rem,
      );
    }
  }

  const addBuf = add ? toUint8Array(add) : undefined;
  const remBuf = rem ? toUint8Array(rem) : undefined;

  if (addBuf) {
    // add must be non-zero; a zero modulus would cause division by zero in the
    // Rust generator.
    const addBitCount = bitCount(addBuf);
    if (addBitCount === 0) {
      throw new NodeRangeError("ERR_OUT_OF_RANGE", "invalid options.add");
    }
    // Node.js/OpenSSL: bit count of add must not exceed requested size
    if (addBitCount > size) {
      throw new NodeRangeError("ERR_OUT_OF_RANGE", "invalid options.add");
    }

    if (remBuf) {
      // rem must be strictly less than add
      const addBigInt = bufferToBigInt(addBuf);
      const remBigInt = bufferToBigInt(remBuf);
      if (addBigInt <= remBigInt) {
        throw new NodeRangeError("ERR_OUT_OF_RANGE", "invalid options.rem");
      }
    }
  }

  return {
    safe,
    bigint,
    add: addBuf,
    rem: remBuf,
  };
}

function bigintToBytes(n: bigint): Uint8Array {
  if (n === 0n) return new Uint8Array([0]);
  const hex = n.toString(16);
  const padded = hex.length % 2 ? "0" + hex : hex;
  const bytes = new Uint8Array(padded.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(padded.substring(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

function bufferToBigInt(buf: Uint8Array): bigint {
  let result = 0n;
  for (let i = 0; i < buf.length; i++) {
    result = (result << 8n) | BigInt(buf[i]);
  }
  return result;
}

function bitCount(buf: Uint8Array): number {
  // Count the number of significant bits in a big-endian byte array
  for (let i = 0; i < buf.length; i++) {
    if (buf[i] !== 0) {
      return (buf.length - i) * 8 - Math.clz32(buf[i]) + 24;
    }
  }
  return 0;
}

/**
 * 48 is the ASCII code for '0', 97 is the ASCII code for 'a'.
 * @param {number} number An integer between 0 and 15.
 * @returns {number} corresponding to the ASCII code of the hex representation
 *                   of the parameter.
 */
const numberToHexCharCode = (number: number): number =>
  (number < 10 ? 48 : 87) + number;

/**
 * @param {ArrayBuffer} buf An ArrayBuffer.
 * @return {bigint}
 */
function arrayBufferToUnsignedBigInt(buf: ArrayBuffer): bigint {
  const length = buf.byteLength;
  const chars: number[] = Array(length * 2);
  const view = new DataView(buf);

  for (let i = 0; i < length; i++) {
    const val = view.getUint8(i);
    chars[2 * i] = numberToHexCharCode(val >> 4);
    chars[2 * i + 1] = numberToHexCharCode(val & 0xf);
  }

  return BigInt(`0x${String.fromCharCode(...chars)}`);
}

function unsignedBigIntToBuffer(bigint: bigint, name: string) {
  if (bigint < 0) {
    throw new ERR_OUT_OF_RANGE(name, ">= 0", bigint);
  }

  const hex = bigint.toString(16);
  const padded = hex.padStart(hex.length + (hex.length % 2), "0");
  return Buffer.from(padded, "hex");
}

export function randomUUID(options) {
  if (options !== undefined) {
    validateObject(options, "options");
  }
  const {
    disableEntropyCache = false,
  } = options || {};

  validateBoolean(disableEntropyCache, "options.disableEntropyCache");

  return globalThis.crypto.randomUUID();
}

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

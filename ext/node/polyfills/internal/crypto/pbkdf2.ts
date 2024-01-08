// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { Buffer } from "node:buffer";
import { HASH_DATA } from "ext:deno_node/internal/crypto/types.ts";

const { core } = globalThis.__bootstrap;
const { ops } = core;
const { op_node_pbkdf2_async } = core.ensureFastOps();

export const MAX_ALLOC = Math.pow(2, 30) - 1;

export type NormalizedAlgorithms =
  | "md5"
  | "ripemd160"
  | "sha1"
  | "sha224"
  | "sha256"
  | "sha384"
  | "sha512";

export type Algorithms =
  | "md5"
  | "ripemd160"
  | "rmd160"
  | "sha1"
  | "sha224"
  | "sha256"
  | "sha384"
  | "sha512";

/**
 * @param iterations Needs to be higher or equal than zero
 * @param keylen  Needs to be higher or equal than zero but less than max allocation size (2^30)
 * @param digest Algorithm to be used for encryption
 */
export function pbkdf2Sync(
  password: HASH_DATA,
  salt: HASH_DATA,
  iterations: number,
  keylen: number,
  digest: Algorithms = "sha1",
): Buffer {
  if (typeof iterations !== "number" || iterations < 0) {
    throw new TypeError("Bad iterations");
  }
  if (typeof keylen !== "number" || keylen < 0 || keylen > MAX_ALLOC) {
    throw new TypeError("Bad key length");
  }

  const DK = new Uint8Array(keylen);
  if (!ops.op_node_pbkdf2(password, salt, iterations, digest, DK)) {
    throw new Error("Invalid digest");
  }

  return Buffer.from(DK);
}

/**
 * @param iterations Needs to be higher or equal than zero
 * @param keylen  Needs to be higher or equal than zero but less than max allocation size (2^30)
 * @param digest Algorithm to be used for encryption
 */
export function pbkdf2(
  password: HASH_DATA,
  salt: HASH_DATA,
  iterations: number,
  keylen: number,
  digest: Algorithms = "sha1",
  callback: (err: Error | null, derivedKey?: Buffer) => void,
) {
  if (typeof iterations !== "number" || iterations < 0) {
    throw new TypeError("Bad iterations");
  }
  if (typeof keylen !== "number" || keylen < 0 || keylen > MAX_ALLOC) {
    throw new TypeError("Bad key length");
  }

  op_node_pbkdf2_async(
    password,
    salt,
    iterations,
    digest,
    keylen,
  ).then(
    (DK) => callback(null, Buffer.from(DK)),
  )
    .catch((err) => callback(err));
}

export default {
  MAX_ALLOC,
  pbkdf2,
  pbkdf2Sync,
};

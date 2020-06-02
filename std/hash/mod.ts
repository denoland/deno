// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { Hash } from "./_hash.ts";
export * from "./hash.ts";

export type SupportedAlgorithm =
  | "md2"
  | "md4"
  | "md5"
  | "ripemd160"
  | "ripemd320"
  | "sha1"
  | "sha224"
  | "sha256"
  | "sha384"
  | "sha512"
  | "sha3-224"
  | "sha3-256"
  | "sha3-384"
  | "sha3-512"
  | "keccak224"
  | "keccak256"
  | "keccak384"
  | "keccak512";

/**
 * Creates a new `Hash` instance.
 * User must call `Hash.dispose` to release all resources.
 *
 * @param algorithm name of hash algorithm to use
 */
export function createHash(algorithm: SupportedAlgorithm): Hash {
  return new Hash(algorithm);
}

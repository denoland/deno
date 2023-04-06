// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

import { notImplemented } from "ext:deno_node/_utils.ts";
import { Buffer } from "ext:deno_node/buffer.ts";
import {
  ERR_INVALID_ARG_TYPE,
  hideStackFrames,
} from "ext:deno_node/internal/errors.ts";
import {
  isAnyArrayBuffer,
  isArrayBufferView,
} from "ext:deno_node/internal/util/types.ts";
import { crypto as constants } from "ext:deno_node/internal_binding/constants.ts";
import {
  kHandle,
  kKeyObject,
} from "ext:deno_node/internal/crypto/constants.ts";

// TODO(kt3k): Generate this list from `digestAlgorithms`
// of std/crypto/_wasm/mod.ts
const digestAlgorithms = [
  "blake2b256",
  "blake2b384",
  "blake2b",
  "blake2s",
  "blake3",
  "keccak-224",
  "keccak-256",
  "keccak-384",
  "keccak-512",
  "sha384",
  "sha3-224",
  "sha3-256",
  "sha3-384",
  "sha3-512",
  "shake128",
  "shake256",
  "tiger",
  "rmd160",
  "sha224",
  "sha256",
  "sha512",
  "md4",
  "md5",
  "sha1",
];

// deno-fmt-ignore
const supportedCiphers = [
  "aes-128-ecb",  "aes-192-ecb",
  "aes-256-ecb",  "aes-128-cbc",
  "aes-192-cbc",  "aes-256-cbc",
  "aes128",       "aes192",
  "aes256",       "aes-128-cfb",
  "aes-192-cfb",  "aes-256-cfb",
  "aes-128-cfb8", "aes-192-cfb8",
  "aes-256-cfb8", "aes-128-cfb1",
  "aes-192-cfb1", "aes-256-cfb1",
  "aes-128-ofb",  "aes-192-ofb",
  "aes-256-ofb",  "aes-128-ctr",
  "aes-192-ctr",  "aes-256-ctr",
  "aes-128-gcm",  "aes-192-gcm",
  "aes-256-gcm"
];

export function getCiphers(): string[] {
  return supportedCiphers;
}

let defaultEncoding = "buffer";

export function setDefaultEncoding(val: string) {
  defaultEncoding = val;
}

export function getDefaultEncoding(): string {
  return defaultEncoding;
}

// This is here because many functions accepted binary strings without
// any explicit encoding in older versions of node, and we don't want
// to break them unnecessarily.
export function toBuf(val: string | Buffer, encoding?: string): Buffer {
  if (typeof val === "string") {
    if (encoding === "buffer") {
      encoding = "utf8";
    }

    return Buffer.from(val, encoding);
  }

  return val;
}

export const validateByteSource = hideStackFrames((val, name) => {
  val = toBuf(val);

  if (isAnyArrayBuffer(val) || isArrayBufferView(val)) {
    return;
  }

  throw new ERR_INVALID_ARG_TYPE(
    name,
    ["string", "ArrayBuffer", "TypedArray", "DataView", "Buffer"],
    val,
  );
});

/**
 * Returns an array of the names of the supported hash algorithms, such as 'sha1'.
 */
export function getHashes(): readonly string[] {
  return digestAlgorithms;
}

export function getCurves(): readonly string[] {
  notImplemented("crypto.getCurves");
}

export interface SecureHeapUsage {
  total: number;
  min: number;
  used: number;
  utilization: number;
}

export function secureHeapUsed(): SecureHeapUsage {
  notImplemented("crypto.secureHeapUsed");
}

export function setEngine(_engine: string, _flags: typeof constants) {
  notImplemented("crypto.setEngine");
}

const kAesKeyLengths = [128, 192, 256];

export { kAesKeyLengths, kHandle, kKeyObject };

export default {
  getDefaultEncoding,
  getHashes,
  setDefaultEncoding,
  getCiphers,
  getCurves,
  secureHeapUsed,
  setEngine,
  validateByteSource,
  toBuf,
  kHandle,
  kKeyObject,
  kAesKeyLengths,
};

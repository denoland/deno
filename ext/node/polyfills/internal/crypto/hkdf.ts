// Copyright 2018-2025 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import {
  op_node_get_hash_size,
  op_node_hkdf,
  op_node_hkdf_async,
} from "ext:core/ops";

import {
  validateFunction,
  validateInteger,
  validateString,
} from "ext:deno_node/internal/validators.mjs";
import {
  ERR_CRYPTO_INVALID_DIGEST,
  ERR_CRYPTO_INVALID_KEYLEN,
  ERR_INVALID_ARG_TYPE,
  ERR_OUT_OF_RANGE,
  hideStackFrames,
} from "ext:deno_node/internal/errors.ts";
import {
  kHandle,
  toBuf,
  validateByteSource,
} from "ext:deno_node/internal/crypto/util.ts";
import {
  createSecretKey,
  KeyObject,
} from "ext:deno_node/internal/crypto/keys.ts";
import type { BinaryLike } from "ext:deno_node/internal/crypto/types.ts";
import { kMaxLength } from "ext:deno_node/internal/buffer.mjs";
import {
  isAnyArrayBuffer,
  isArrayBufferView,
} from "ext:deno_node/internal/util/types.ts";
import { isKeyObject } from "ext:deno_node/internal/crypto/_keys.ts";
import { getHashes } from "ext:deno_node/internal/crypto/hash.ts";
import { Buffer } from "node:buffer";

// Consume raw bytes for any ArrayBufferView/ArrayBuffer; strings via toBuf.
function toRawBytes(x: unknown): Buffer {
  if (isArrayBufferView(x)) {
    const v = x as ArrayBufferView;
    return Buffer.from(v.buffer, v.byteOffset, v.byteLength);
  }
  if (isAnyArrayBuffer(x)) {
    return Buffer.from(x as ArrayBufferLike);
  }
  // For strings / other BinaryLike, keep existing semantics (UTF-8 etc.)
  return Buffer.from(toBuf(x as unknown as string));
}

const validateParameters = hideStackFrames((hash, key, salt, info, length) => {
  validateString(hash, "digest");
  key = prepareKey(key);
  validateByteSource(salt, "salt");
  validateByteSource(info, "info");

  salt = toRawBytes(toBuf(salt));
  info = toRawBytes(toBuf(info));

  validateInteger(length, "length", 0, kMaxLength);

  if (info.byteLength > 1024) {
    throw new ERR_OUT_OF_RANGE(
      "info",
      "must not contain more than 1024 bytes",
      info.byteLength,
    );
  }

  validateAlgorithm(hash);

  const size = op_node_get_hash_size(hash);
  if (typeof size === "number" && size * 255 < length) {
    throw new ERR_CRYPTO_INVALID_KEYLEN();
  }

  return {
    hash,
    key,
    salt,
    info,
    length,
  };
});

function prepareKey(key: BinaryLike | KeyObject) {
  if (isKeyObject(key)) {
    return key;
  }

  if (isAnyArrayBuffer(key)) {
    return createSecretKey(new Uint8Array(key as unknown as ArrayBufferLike));
  }

  key = toBuf(key as string);

  if (!isArrayBufferView(key)) {
    throw new ERR_INVALID_ARG_TYPE(
      "ikm",
      [
        "string",
        "SecretKeyObject",
        "ArrayBuffer",
        "TypedArray",
        "DataView",
        "Buffer",
      ],
      key,
    );
  }

  return createSecretKey(key);
}

export function hkdf(
  hash: string,
  key: BinaryLike | KeyObject,
  salt: BinaryLike,
  info: BinaryLike,
  length: number,
  callback: (err: Error | null, derivedKey: ArrayBuffer | undefined) => void,
) {
  ({ hash, key, salt, info, length } = validateParameters(
    hash,
    key,
    salt,
    info,
    length,
  ));

  validateFunction(callback, "callback");

  hash = hash.toLowerCase();

  op_node_hkdf_async(hash, key[kHandle], salt, info, length)
    .then((okm) => callback(null, okm.buffer))
    .catch((err) => callback(new ERR_CRYPTO_INVALID_DIGEST(err), undefined));
}

export function hkdfSync(
  hash: string,
  key: BinaryLike | KeyObject,
  salt: BinaryLike,
  info: BinaryLike,
  length: number,
) {
  ({ hash, key, salt, info, length } = validateParameters(
    hash,
    key,
    salt,
    info,
    length,
  ));

  hash = hash.toLowerCase();

  const okm = new Uint8Array(length);
  try {
    op_node_hkdf(hash, key[kHandle], salt, info, okm);
  } catch (e) {
    throw new ERR_CRYPTO_INVALID_DIGEST(e);
  }

  return okm.buffer;
}

let hashes: Set<string> | null = null;
function validateAlgorithm(algorithm: string) {
  if (hashes === null) {
    hashes = new Set(getHashes());
  }

  if (!hashes.has(algorithm)) {
    throw new ERR_CRYPTO_INVALID_DIGEST(algorithm);
  }
}

export default {
  hkdf,
  hkdfSync,
};

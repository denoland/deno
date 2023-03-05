// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

import {
  validateFunction,
  validateInteger,
  validateString,
} from "internal:deno_node/internal/validators.mjs";
import {
  ERR_INVALID_ARG_TYPE,
  ERR_OUT_OF_RANGE,
  hideStackFrames,
} from "internal:deno_node/internal/errors.ts";
import {
  toBuf,
  validateByteSource,
} from "internal:deno_node/internal/crypto/util.ts";
import {
  createSecretKey,
  isKeyObject,
  KeyObject,
} from "internal:deno_node/internal/crypto/keys.ts";
import type { BinaryLike } from "internal:deno_node/internal/crypto/types.ts";
import { kMaxLength } from "internal:deno_node/internal/buffer.mjs";
import {
  isAnyArrayBuffer,
  isArrayBufferView,
} from "internal:deno_node/internal/util/types.ts";
import { notImplemented } from "internal:deno_node/_utils.ts";

const validateParameters = hideStackFrames((hash, key, salt, info, length) => {
  key = prepareKey(key);
  salt = toBuf(salt);
  info = toBuf(info);

  validateString(hash, "digest");
  validateByteSource(salt, "salt");
  validateByteSource(info, "info");

  validateInteger(length, "length", 0, kMaxLength);

  if (info.byteLength > 1024) {
    throw new ERR_OUT_OF_RANGE(
      "info",
      "must not contain more than 1024 bytes",
      info.byteLength,
    );
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
  callback: (err: Error | null, derivedKey: ArrayBuffer) => void,
) {
  ({ hash, key, salt, info, length } = validateParameters(
    hash,
    key,
    salt,
    info,
    length,
  ));

  validateFunction(callback, "callback");

  notImplemented("crypto.hkdf");
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

  notImplemented("crypto.hkdfSync");
}

export default {
  hkdf,
  hkdfSync,
};

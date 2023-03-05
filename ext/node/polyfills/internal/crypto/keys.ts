// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

import {
  kHandle,
  kKeyObject,
} from "internal:deno_node/internal/crypto/constants.ts";
import {
  ERR_CRYPTO_INVALID_KEY_OBJECT_TYPE,
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_ARG_VALUE,
} from "internal:deno_node/internal/errors.ts";
import { notImplemented } from "internal:deno_node/_utils.ts";
import type {
  KeyFormat,
  KeyType,
  PrivateKeyInput,
  PublicKeyInput,
} from "internal:deno_node/internal/crypto/types.ts";
import { Buffer } from "internal:deno_node/buffer.ts";
import {
  isAnyArrayBuffer,
  isArrayBufferView,
} from "internal:deno_node/internal/util/types.ts";
import { hideStackFrames } from "internal:deno_node/internal/errors.ts";
import {
  isCryptoKey as isCryptoKey_,
  isKeyObject as isKeyObject_,
  kKeyType,
} from "internal:deno_node/internal/crypto/_keys.ts";

const getArrayBufferOrView = hideStackFrames(
  (
    buffer,
    name,
    encoding,
  ):
    | ArrayBuffer
    | SharedArrayBuffer
    | Buffer
    | DataView
    | BigInt64Array
    | BigUint64Array
    | Float32Array
    | Float64Array
    | Int8Array
    | Int16Array
    | Int32Array
    | Uint8Array
    | Uint8ClampedArray
    | Uint16Array
    | Uint32Array => {
    if (isAnyArrayBuffer(buffer)) {
      return buffer;
    }
    if (typeof buffer === "string") {
      if (encoding === "buffer") {
        encoding = "utf8";
      }
      return Buffer.from(buffer, encoding);
    }
    if (!isArrayBufferView(buffer)) {
      throw new ERR_INVALID_ARG_TYPE(
        name,
        [
          "string",
          "ArrayBuffer",
          "Buffer",
          "TypedArray",
          "DataView",
        ],
        buffer,
      );
    }
    return buffer;
  },
);

export interface AsymmetricKeyDetails {
  /**
   * Key size in bits (RSA, DSA).
   */
  modulusLength?: number | undefined;
  /**
   * Public exponent (RSA).
   */
  publicExponent?: bigint | undefined;
  /**
   * Name of the message digest (RSA-PSS).
   */
  hashAlgorithm?: string | undefined;
  /**
   * Name of the message digest used by MGF1 (RSA-PSS).
   */
  mgf1HashAlgorithm?: string | undefined;
  /**
   * Minimal salt length in bytes (RSA-PSS).
   */
  saltLength?: number | undefined;
  /**
   * Size of q in bits (DSA).
   */
  divisorLength?: number | undefined;
  /**
   * Name of the curve (EC).
   */
  namedCurve?: string | undefined;
}

export type KeyObjectType = "secret" | "public" | "private";

export interface KeyExportOptions<T extends KeyFormat> {
  type: "pkcs1" | "spki" | "pkcs8" | "sec1";
  format: T;
  cipher?: string | undefined;
  passphrase?: string | Buffer | undefined;
}

export interface JwkKeyExportOptions {
  format: "jwk";
}

export function isKeyObject(obj: unknown): obj is KeyObject {
  return isKeyObject_(obj);
}

export function isCryptoKey(
  obj: unknown,
): obj is { type: string; [kKeyObject]: KeyObject } {
  return isCryptoKey_(obj);
}

export class KeyObject {
  [kKeyType]: KeyObjectType;
  [kHandle]: unknown;

  constructor(type: KeyObjectType, handle: unknown) {
    if (type !== "secret" && type !== "public" && type !== "private") {
      throw new ERR_INVALID_ARG_VALUE("type", type);
    }

    if (typeof handle !== "object") {
      throw new ERR_INVALID_ARG_TYPE("handle", "object", handle);
    }

    this[kKeyType] = type;

    Object.defineProperty(this, kHandle, {
      value: handle,
      enumerable: false,
      configurable: false,
      writable: false,
    });
  }

  get type(): KeyObjectType {
    return this[kKeyType];
  }

  get asymmetricKeyDetails(): AsymmetricKeyDetails | undefined {
    notImplemented("crypto.KeyObject.prototype.asymmetricKeyDetails");

    return undefined;
  }

  get asymmetricKeyType(): KeyType | undefined {
    notImplemented("crypto.KeyObject.prototype.asymmetricKeyType");

    return undefined;
  }

  get symmetricKeySize(): number | undefined {
    notImplemented("crypto.KeyObject.prototype.symmetricKeySize");

    return undefined;
  }

  static from(key: CryptoKey): KeyObject {
    if (!isCryptoKey(key)) {
      throw new ERR_INVALID_ARG_TYPE("key", "CryptoKey", key);
    }

    notImplemented("crypto.KeyObject.prototype.from");
  }

  equals(otherKeyObject: KeyObject): boolean {
    if (!isKeyObject(otherKeyObject)) {
      throw new ERR_INVALID_ARG_TYPE(
        "otherKeyObject",
        "KeyObject",
        otherKeyObject,
      );
    }

    notImplemented("crypto.KeyObject.prototype.equals");
  }

  export(options: KeyExportOptions<"pem">): string | Buffer;
  export(options?: KeyExportOptions<"der">): Buffer;
  export(options?: JwkKeyExportOptions): JsonWebKey;
  export(_options?: unknown): string | Buffer | JsonWebKey {
    notImplemented("crypto.KeyObject.prototype.asymmetricKeyType");
  }
}

export interface JsonWebKeyInput {
  key: JsonWebKey;
  format: "jwk";
}

export function createPrivateKey(
  _key: PrivateKeyInput | string | Buffer | JsonWebKeyInput,
): KeyObject {
  notImplemented("crypto.createPrivateKey");
}

export function createPublicKey(
  _key: PublicKeyInput | string | Buffer | KeyObject | JsonWebKeyInput,
): KeyObject {
  notImplemented("crypto.createPublicKey");
}

function getKeyTypes(allowKeyObject: boolean, bufferOnly = false) {
  const types = [
    "ArrayBuffer",
    "Buffer",
    "TypedArray",
    "DataView",
    "string", // Only if bufferOnly == false
    "KeyObject", // Only if allowKeyObject == true && bufferOnly == false
    "CryptoKey", // Only if allowKeyObject == true && bufferOnly == false
  ];
  if (bufferOnly) {
    return types.slice(0, 4);
  } else if (!allowKeyObject) {
    return types.slice(0, 5);
  }
  return types;
}

export function prepareSecretKey(
  key: string | ArrayBuffer | KeyObject,
  encoding: string | undefined,
  bufferOnly = false,
) {
  if (!bufferOnly) {
    if (isKeyObject(key)) {
      if (key.type !== "secret") {
        throw new ERR_CRYPTO_INVALID_KEY_OBJECT_TYPE(key.type, "secret");
      }
      return key[kHandle];
    } else if (isCryptoKey(key)) {
      if (key.type !== "secret") {
        throw new ERR_CRYPTO_INVALID_KEY_OBJECT_TYPE(key.type, "secret");
      }
      return key[kKeyObject][kHandle];
    }
  }
  if (
    typeof key !== "string" &&
    !isArrayBufferView(key) &&
    !isAnyArrayBuffer(key)
  ) {
    throw new ERR_INVALID_ARG_TYPE(
      "key",
      getKeyTypes(!bufferOnly, bufferOnly),
      key,
    );
  }

  return getArrayBufferOrView(key, "key", encoding);
}

export function createSecretKey(key: ArrayBufferView): KeyObject;
export function createSecretKey(
  key: string,
  encoding: string,
): KeyObject;
export function createSecretKey(
  _key: string | ArrayBufferView,
  _encoding?: string,
): KeyObject {
  notImplemented("crypto.createSecretKey");
}

export default {
  createPrivateKey,
  createPublicKey,
  createSecretKey,
  isKeyObject,
  isCryptoKey,
  KeyObject,
  prepareSecretKey,
};

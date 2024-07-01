// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { primordials } from "ext:core/mod.js";

const {
  ObjectDefineProperties,
  SymbolToStringTag,
} = primordials;

import {
  op_node_create_private_key,
  op_node_create_public_key,
  op_node_export_rsa_public_pem,
  op_node_export_rsa_spki_der,
} from "ext:core/ops";

import {
  kHandle,
  kKeyObject,
} from "ext:deno_node/internal/crypto/constants.ts";
import { isStringOrBuffer } from "ext:deno_node/internal/crypto/cipher.ts";
import {
  ERR_CRYPTO_INVALID_KEY_OBJECT_TYPE,
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_ARG_VALUE,
} from "ext:deno_node/internal/errors.ts";
import { notImplemented } from "ext:deno_node/_utils.ts";
import type {
  KeyFormat,
  PrivateKeyInput,
  PublicKeyInput,
} from "ext:deno_node/internal/crypto/types.ts";
import { Buffer } from "node:buffer";
import {
  isAnyArrayBuffer,
  isArrayBufferView,
} from "ext:deno_node/internal/util/types.ts";
import { hideStackFrames } from "ext:deno_node/internal/errors.ts";
import {
  isCryptoKey as isCryptoKey_,
  isKeyObject as isKeyObject_,
  kKeyType,
} from "ext:deno_node/internal/crypto/_keys.ts";
import {
  validateObject,
  validateOneOf,
} from "ext:deno_node/internal/validators.mjs";
import {
  forgivingBase64UrlEncode as encodeToBase64Url,
} from "ext:deno_web/00_infra.js";

export const getArrayBufferOrView = hideStackFrames(
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
      return new Uint8Array(buffer);
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

function copyBuffer(input: string | Buffer | ArrayBufferView) {
  if (typeof input === "string") return Buffer.from(input);
  return (
    (ArrayBuffer.isView(input)
      ? new Uint8Array(input.buffer, input.byteOffset, input.byteLength)
      : new Uint8Array(input)).slice()
  );
}

const KEY_STORE = new WeakMap();

export class KeyObject {
  [kKeyType]: KeyObjectType;
  [kHandle]: unknown;

  constructor(type: KeyObjectType, handle: unknown) {
    if (type !== "secret" && type !== "public" && type !== "private") {
      throw new ERR_INVALID_ARG_VALUE("type", type);
    }

    this[kKeyType] = type;
    this[kHandle] = handle;
  }

  get type(): KeyObjectType {
    return this[kKeyType];
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

ObjectDefineProperties(KeyObject.prototype, {
  [SymbolToStringTag]: {
    __proto__: null,
    configurable: true,
    value: "KeyObject",
  },
});

export interface JsonWebKeyInput {
  key: JsonWebKey;
  format: "jwk";
}

export function prepareAsymmetricKey(key) {
  if (isStringOrBuffer(key)) {
    return { format: "pem", data: getArrayBufferOrView(key, "key") };
  } else if (isKeyObject(key)) {
    return {
      // Assumes that assymetric keys are stored as PEM.
      format: "pem",
      data: getKeyMaterial(key),
    };
  } else if (typeof key == "object") {
    const { key: data, encoding, format, type } = key;
    if (!isStringOrBuffer(data)) {
      throw new TypeError("Invalid key type");
    }

    return {
      data: getArrayBufferOrView(data, "key", encoding),
      format: format ?? "pem",
      encoding,
      type,
    };
  }

  throw new TypeError("Invalid key type");
}

export function createPrivateKey(
  key: PrivateKeyInput | string | Buffer | JsonWebKeyInput,
): PrivateKeyObject {
  const { data, format, type } = prepareAsymmetricKey(key);
  const details = op_node_create_private_key(data, format, type);
  const handle = setOwnedKey(copyBuffer(data));
  return new PrivateKeyObject(handle, details);
}

export function createPublicKey(
  key: PublicKeyInput | string | Buffer | JsonWebKeyInput,
): PublicKeyObject {
  const { data, format, type } = prepareAsymmetricKey(key);
  const details = op_node_create_public_key(data, format, type);
  const handle = setOwnedKey(copyBuffer(data));
  return new PublicKeyObject(handle, details);
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
  key: string | ArrayBufferView | ArrayBuffer | KeyObject,
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

export class SecretKeyObject extends KeyObject {
  constructor(handle: unknown) {
    super("secret", handle);
  }

  get symmetricKeySize() {
    return KEY_STORE.get(this[kHandle]).byteLength;
  }

  get asymmetricKeyType() {
    return undefined;
  }

  export(): Buffer;
  export(options?: JwkKeyExportOptions): JsonWebKey {
    const key = KEY_STORE.get(this[kHandle]);
    if (options !== undefined) {
      validateObject(options, "options");
      validateOneOf(
        options.format,
        "options.format",
        [undefined, "buffer", "jwk"],
      );
      if (options.format === "jwk") {
        return {
          kty: "oct",
          k: encodeToBase64Url(key),
        };
      }
    }
    return key.slice();
  }
}

const kAsymmetricKeyType = Symbol("kAsymmetricKeyType");
const kAsymmetricKeyDetails = Symbol("kAsymmetricKeyDetails");

class AsymmetricKeyObject extends KeyObject {
  constructor(type: KeyObjectType, handle: unknown, details: unknown) {
    super(type, handle);
    this[kAsymmetricKeyType] = details.type;
    this[kAsymmetricKeyDetails] = { ...details };
  }

  get asymmetricKeyType() {
    return this[kAsymmetricKeyType];
  }

  get asymmetricKeyDetails() {
    return this[kAsymmetricKeyDetails];
  }
}

export class PrivateKeyObject extends AsymmetricKeyObject {
  constructor(handle: unknown, details: unknown) {
    super("private", handle, details);
  }

  export(_options: unknown) {
    notImplemented("crypto.PrivateKeyObject.prototype.export");
  }
}

export class PublicKeyObject extends AsymmetricKeyObject {
  constructor(handle: unknown, details: unknown) {
    super("public", handle, details);
  }

  export(options: unknown) {
    const key = KEY_STORE.get(this[kHandle]);
    switch (this.asymmetricKeyType) {
      case "rsa":
      case "rsa-pss": {
        switch (options.format) {
          case "pem":
            return op_node_export_rsa_public_pem(key);
          case "der": {
            if (options.type == "pkcs1") {
              return key;
            } else {
              return op_node_export_rsa_spki_der(key);
            }
          }
          default:
            throw new TypeError(`exporting ${options.type} is not implemented`);
        }
      }
      default:
        throw new TypeError(
          `exporting ${this.asymmetricKeyType} is not implemented`,
        );
    }
  }
}

export function setOwnedKey(key: Uint8Array): unknown {
  const handle = {};
  KEY_STORE.set(handle, key);
  return handle;
}

export function getKeyMaterial(key: KeyObject): Uint8Array {
  return KEY_STORE.get(key[kHandle]);
}

export function createSecretKey(key: ArrayBufferView): KeyObject;
export function createSecretKey(
  key: string,
  encoding: string,
): KeyObject;
export function createSecretKey(
  key: string | ArrayBufferView,
  encoding?: string,
): KeyObject {
  key = prepareSecretKey(key, encoding, true);
  const handle = setOwnedKey(copyBuffer(key));
  return new SecretKeyObject(handle);
}

export default {
  createPrivateKey,
  createPublicKey,
  createSecretKey,
  isKeyObject,
  isCryptoKey,
  KeyObject,
  prepareSecretKey,
  setOwnedKey,
  SecretKeyObject,
  PrivateKeyObject,
  PublicKeyObject,
};

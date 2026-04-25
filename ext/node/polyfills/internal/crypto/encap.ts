// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { primordials } from "ext:core/mod.js";
import { op_node_rsa_decapsulate, op_node_rsa_encapsulate } from "ext:core/ops";

import { Buffer } from "node:buffer";
import {
  isAnyArrayBuffer,
  isArrayBufferView,
} from "ext:deno_node/internal/util/types.ts";
import {
  createPrivateKey,
  createPublicKey,
  getArrayBufferOrView,
  type KeyObject,
} from "ext:deno_node/internal/crypto/keys.ts";
import {
  isCryptoKey,
  isKeyObject,
} from "ext:deno_node/internal/crypto/_keys.ts";
import { NodeError } from "ext:deno_node/internal/errors.ts";

const { FunctionPrototypeCall } = primordials;

type EncapsulateCallback = (
  err: Error | null,
  result?: { sharedKey: Buffer; ciphertext: Buffer },
) => void;
type DecapsulateCallback = (err: Error | null, result?: Buffer) => void;

/**
 * Validate that the value is a recognised asymmetric-key input shape. Throws
 * ERR_INVALID_ARG_TYPE if not -- distinct from a key that is well-formed but
 * fails to parse as RSA, which throws ERR_OSSL_EVP_DECODE_ERROR later.
 */
function validateKeyInput(key: unknown): void {
  if (
    typeof key === "string" ||
    isAnyArrayBuffer(key) ||
    isArrayBufferView(key) ||
    isKeyObject(key) ||
    isCryptoKey(key) ||
    (typeof key === "object" && key !== null)
  ) {
    return;
  }
  let received: string;
  if (key === null) {
    received = "null";
  } else if (key === undefined) {
    received = "undefined";
  } else {
    received = `type ${typeof key}`;
  }
  throw new NodeError(
    "ERR_INVALID_ARG_TYPE",
    `The "key" argument must be of type ArrayBuffer, Buffer, TypedArray, ` +
      `DataView, string, KeyObject, or CryptoKey. Received ${received}`,
  );
}

/**
 * Normalize an asymmetric key input (KeyObject, PEM string, DER buffer, JWK
 * `{ key, format: 'jwk' }`, or `{ key, format: 'der', type: 'spki'|'pkcs8' }`)
 * to PEM/DER bytes the Rust ops can parse.
 *
 * For JWK and explicit `{key, format, type}` shapes we route through
 * `createPublicKey`/`createPrivateKey` to do the heavy lifting, then export
 * back to SPKI/PKCS8 PEM.
 */
function asymmetricKeyToBytes(
  key: unknown,
  isPublic: boolean,
): Uint8Array {
  // KeyObject: export to standard PEM.
  if (isKeyObject(key)) {
    const ko = key as KeyObject;
    const data = isPublic
      ? ko.export({ type: "spki", format: "pem" })
      : ko.export({ type: "pkcs8", format: "pem" });
    return getArrayBufferOrView(data, "key");
  }

  if (isCryptoKey(key)) {
    // Round-trip through createPublic/PrivateKey.
    const ko = isPublic ? createPublicKey(key) : createPrivateKey(key);
    return asymmetricKeyToBytes(ko, isPublic);
  }

  // Raw string / Buffer / TypedArray: pass through as-is (PEM or DER bytes).
  if (
    typeof key === "string" || isArrayBufferView(key) || isAnyArrayBuffer(key)
  ) {
    return getArrayBufferOrView(key, "key");
  }

  // Object form: { key, format?, type?, encoding? }.
  if (key !== null && typeof key === "object") {
    const obj = key as { key: unknown; format?: string; encoding?: string };
    const inner = obj.key;
    if (isKeyObject(inner)) {
      return asymmetricKeyToBytes(inner, isPublic);
    }
    // JWK or DER-via-options: route through createPublic/PrivateKey to
    // normalize, then re-export. createPublicKey/createPrivateKey handle JWK,
    // PEM, and DER+type combinations.
    const ko = isPublic
      ? createPublicKey(obj as Parameters<typeof createPublicKey>[0])
      : createPrivateKey(obj as Parameters<typeof createPrivateKey>[0]);
    return asymmetricKeyToBytes(ko, isPublic);
  }

  throw new NodeError("ERR_INVALID_ARG_TYPE", "Invalid key type");
}

function ciphertextTypeMessage(value: unknown): string {
  let received: string;
  if (value === null) {
    received = "null";
  } else if (value === undefined) {
    received = "undefined";
  } else {
    const type = typeof value;
    if (type === "object") {
      const ctor = (value as { constructor?: { name?: string } }).constructor;
      received = `an instance of ${ctor?.name ?? "Object"}`;
    } else {
      received = `type ${type}`;
    }
  }
  return `The "ciphertext" argument must be an instance of ArrayBuffer, ` +
    `Buffer, TypedArray, or DataView. Received ${received}`;
}

function decodeError(): Error {
  return new NodeError("ERR_OSSL_EVP_DECODE_ERROR", "Failed to decode key");
}

function operationFailed(stage: "initialize" | "perform", op: string): Error {
  return new NodeError(
    "ERR_CRYPTO_OPERATION_FAILED",
    `Failed to ${stage} ${op}`,
  );
}

function asyncFailure(): Error {
  // Async failure path uses a fixed message per Node.
  return new NodeError("ERR_CRYPTO_OPERATION_FAILED", "Deriving bits failed");
}

function encapsulateImpl(
  publicKey: unknown,
): { sharedKey: Buffer; ciphertext: Buffer } {
  validateKeyInput(publicKey);
  let keyBytes: Uint8Array;
  try {
    keyBytes = asymmetricKeyToBytes(publicKey, /* isPublic */ true);
  } catch {
    throw decodeError();
  }
  let result: [Uint8Array, Uint8Array];
  try {
    result = op_node_rsa_encapsulate(keyBytes);
  } catch {
    throw decodeError();
  }
  return {
    sharedKey: Buffer.from(
      result[0].buffer,
      result[0].byteOffset,
      result[0].byteLength,
    ),
    ciphertext: Buffer.from(
      result[1].buffer,
      result[1].byteOffset,
      result[1].byteLength,
    ),
  };
}

function decapsulateImpl(
  privateKey: unknown,
  ciphertext: Uint8Array,
): Buffer {
  validateKeyInput(privateKey);
  let keyBytes: Uint8Array;
  try {
    keyBytes = asymmetricKeyToBytes(privateKey, /* isPublic */ false);
  } catch {
    throw operationFailed("initialize", "decapsulation");
  }
  let result: Uint8Array;
  try {
    result = op_node_rsa_decapsulate(keyBytes, ciphertext);
  } catch {
    throw operationFailed("perform", "decapsulation");
  }
  return Buffer.from(result.buffer, result.byteOffset, result.byteLength);
}

export function encapsulate(
  publicKey: unknown,
  optionsOrCallback?: unknown,
  maybeCallback?: EncapsulateCallback,
): { sharedKey: Buffer; ciphertext: Buffer } | undefined {
  let callback: EncapsulateCallback | undefined;
  if (typeof optionsOrCallback === "function") {
    callback = optionsOrCallback as EncapsulateCallback;
  } else if (
    optionsOrCallback !== undefined && optionsOrCallback !== null &&
    typeof optionsOrCallback !== "object"
  ) {
    throw new NodeError(
      "ERR_INVALID_ARG_TYPE",
      `The "options" argument must be of type object. Received ${typeof optionsOrCallback}`,
    );
  } else {
    callback = maybeCallback;
  }

  if (callback !== undefined && typeof callback !== "function") {
    throw new NodeError(
      "ERR_INVALID_ARG_TYPE",
      `The "callback" argument must be of type function. Received ${typeof callback}`,
    );
  }

  if (callback) {
    const cb = callback;
    queueMicrotask(() => {
      try {
        const result = encapsulateImpl(publicKey);
        FunctionPrototypeCall(cb, undefined, null, result);
      } catch {
        FunctionPrototypeCall(cb, undefined, asyncFailure());
      }
    });
    return undefined;
  }
  return encapsulateImpl(publicKey);
}

export function decapsulate(
  privateKey: unknown,
  ciphertext: unknown,
  optionsOrCallback?: unknown,
  maybeCallback?: DecapsulateCallback,
): Buffer | undefined {
  // Validate `key` first so zero-arg `decapsulate()` reports the key error,
  // matching Node's argument-validation order.
  validateKeyInput(privateKey);
  if (
    !isAnyArrayBuffer(ciphertext) && !isArrayBufferView(ciphertext)
  ) {
    throw new NodeError(
      "ERR_INVALID_ARG_TYPE",
      ciphertextTypeMessage(ciphertext),
    );
  }
  // After the type check, ciphertext is guaranteed to be ArrayBuffer or view.
  const ctView = getArrayBufferOrView(
    ciphertext as ArrayBuffer | ArrayBufferView,
    "ciphertext",
  );

  let callback: DecapsulateCallback | undefined;
  if (typeof optionsOrCallback === "function") {
    callback = optionsOrCallback as DecapsulateCallback;
  } else if (
    optionsOrCallback !== undefined && optionsOrCallback !== null &&
    typeof optionsOrCallback !== "object"
  ) {
    throw new NodeError(
      "ERR_INVALID_ARG_TYPE",
      `The "options" argument must be of type object. Received ${typeof optionsOrCallback}`,
    );
  } else {
    callback = maybeCallback;
  }

  if (callback !== undefined && typeof callback !== "function") {
    throw new NodeError(
      "ERR_INVALID_ARG_TYPE",
      `The "callback" argument must be of type function. Received ${typeof callback}`,
    );
  }

  if (callback) {
    const cb = callback;
    queueMicrotask(() => {
      try {
        const result = decapsulateImpl(privateKey, ctView);
        FunctionPrototypeCall(cb, undefined, null, result);
      } catch {
        FunctionPrototypeCall(cb, undefined, asyncFailure());
      }
    });
    return undefined;
  }
  return decapsulateImpl(privateKey, ctView);
}

export default { encapsulate, decapsulate };

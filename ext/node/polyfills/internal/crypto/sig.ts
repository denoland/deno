// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { op_node_sign, op_node_verify } from "ext:core/ops";

import { notImplemented } from "ext:deno_node/_utils.ts";
import {
  validateFunction,
  validateString,
} from "ext:deno_node/internal/validators.mjs";
import { Buffer } from "node:buffer";
import type { WritableOptions } from "ext:deno_node/_stream.d.ts";
import Writable from "ext:deno_node/internal/streams/writable.mjs";
import type {
  BinaryLike,
  BinaryToTextEncoding,
  Encoding,
  PrivateKeyInput,
  PublicKeyInput,
} from "ext:deno_node/internal/crypto/types.ts";
import {
  KeyObject,
  prepareAsymmetricKey,
} from "ext:deno_node/internal/crypto/keys.ts";
import { createHash, Hash } from "ext:deno_node/internal/crypto/hash.ts";
import { KeyFormat, KeyType } from "ext:deno_node/internal/crypto/types.ts";
import { isArrayBufferView } from "ext:deno_node/internal/util/types.ts";
import { ERR_CRYPTO_SIGN_KEY_REQUIRED } from "ext:deno_node/internal/errors.ts";

export type DSAEncoding = "der" | "ieee-p1363";

export interface SigningOptions {
  padding?: number | undefined;
  saltLength?: number | undefined;
  dsaEncoding?: DSAEncoding | undefined;
}

export interface SignPrivateKeyInput extends PrivateKeyInput, SigningOptions {}

export interface SignKeyObjectInput extends SigningOptions {
  key: KeyObject;
}
export interface VerifyPublicKeyInput extends PublicKeyInput, SigningOptions {}

export interface VerifyKeyObjectInput extends SigningOptions {
  key: KeyObject;
}

export type KeyLike = string | Buffer | KeyObject;

export class SignImpl extends Writable {
  hash: Hash;
  #digestType: string;

  constructor(algorithm: string, _options?: WritableOptions) {
    validateString(algorithm, "algorithm");

    super({
      write(chunk, enc, callback) {
        this.update(chunk, enc);
        callback();
      },
    });

    algorithm = algorithm.toLowerCase();

    this.#digestType = algorithm;
    this.hash = createHash(this.#digestType);
  }

  sign(
    privateKey: BinaryLike | SignKeyObjectInput | SignPrivateKeyInput,
    encoding?: BinaryToTextEncoding,
  ): Buffer | string {
    const { data, format, type } = prepareAsymmetricKey(privateKey);
    const ret = Buffer.from(op_node_sign(
      this.hash.digest(),
      this.#digestType,
      data!,
      type,
      format,
    ));
    return encoding ? ret.toString(encoding) : ret;
  }

  update(
    data: BinaryLike | string,
    encoding?: Encoding,
  ): this {
    this.hash.update(data, encoding);
    return this;
  }
}

export function Sign(algorithm: string, options?: WritableOptions) {
  return new SignImpl(algorithm, options);
}

Sign.prototype = SignImpl.prototype;

export class VerifyImpl extends Writable {
  hash: Hash;
  #digestType: string;

  constructor(algorithm: string, _options?: WritableOptions) {
    validateString(algorithm, "algorithm");

    super({
      write(chunk, enc, callback) {
        this.update(chunk, enc);
        callback();
      },
    });

    algorithm = algorithm.toLowerCase();

    this.#digestType = algorithm;
    this.hash = createHash(this.#digestType);
  }

  update(data: BinaryLike, encoding?: string): this {
    this.hash.update(data, encoding);
    return this;
  }

  verify(
    publicKey: BinaryLike | VerifyKeyObjectInput | VerifyPublicKeyInput,
    signature: BinaryLike,
    encoding?: BinaryToTextEncoding,
  ): boolean {
    let keyData: BinaryLike;
    let keyType: KeyType;
    let keyFormat: KeyFormat;
    if (typeof publicKey === "string" || isArrayBufferView(publicKey)) {
      // if the key is BinaryLike, interpret it as a PEM encoded RSA key
      // deno-lint-ignore no-explicit-any
      keyData = publicKey as any;
      keyType = "rsa";
      keyFormat = "pem";
    } else {
      // TODO(kt3k): Add support for the case when publicKey is a KeyObject,
      // CryptoKey, etc
      notImplemented(
        "crypto.Verify.prototype.verify with non BinaryLike input",
      );
    }
    return op_node_verify(
      this.hash.digest(),
      this.#digestType,
      keyData!,
      keyType,
      keyFormat,
      Buffer.from(signature, encoding),
    );
  }
}

export function Verify(algorithm: string, options?: WritableOptions) {
  return new VerifyImpl(algorithm, options);
}

Verify.prototype = VerifyImpl.prototype;

export function signOneShot(
  algorithm: string | null | undefined,
  data: ArrayBufferView,
  key: KeyLike | SignKeyObjectInput | SignPrivateKeyInput,
  callback?: (error: Error | null, data: Buffer) => void,
): Buffer | void {
  if (algorithm != null) {
    validateString(algorithm, "algorithm");
  }

  if (callback !== undefined) {
    validateFunction(callback, "callback");
  }

  if (!key) {
    throw new ERR_CRYPTO_SIGN_KEY_REQUIRED();
  }

  const result = Sign(algorithm!).update(data).sign(key);

  if (callback) {
    setTimeout(() => callback(null, result));
  } else {
    return result;
  }
}

export function verifyOneShot(
  algorithm: string | null | undefined,
  data: BinaryLike,
  key: KeyLike | VerifyKeyObjectInput | VerifyPublicKeyInput,
  signature: BinaryLike,
  callback?: (error: Error | null, result: boolean) => void,
): boolean | void {
  if (algorithm != null) {
    validateString(algorithm, "algorithm");
  }

  if (callback !== undefined) {
    validateFunction(callback, "callback");
  }

  if (!key) {
    throw new ERR_CRYPTO_SIGN_KEY_REQUIRED();
  }

  const result = Verify(algorithm!).update(data).verify(key, signature);

  if (callback) {
    setTimeout(() => callback(null, result));
  } else {
    return result;
  }
}

export default {
  signOneShot,
  verifyOneShot,
  Sign,
  Verify,
};

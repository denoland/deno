// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import {
  op_node_create_private_key,
  op_node_create_public_key,
  op_node_get_asymmetric_key_type,
  op_node_sign,
  op_node_verify,
} from "ext:core/ops";

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
  kConsumePrivate,
  kConsumePublic,
  KeyObject,
  prepareAsymmetricKey,
  PrivateKeyObject,
  PublicKeyObject,
} from "ext:deno_node/internal/crypto/keys.ts";
import { createHash } from "ext:deno_node/internal/crypto/hash.ts";
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
    // deno-lint-ignore no-explicit-any
    privateKey: any,
    encoding?: BinaryToTextEncoding,
  ): Buffer | string {
    const res = prepareAsymmetricKey(privateKey, kConsumePrivate);
    let handle;
    if ("handle" in res) {
      handle = res.handle;
    } else {
      handle = op_node_create_private_key(
        res.data,
        res.format,
        res.type ?? "",
        res.passphrase,
      );
    }
    const ret = Buffer.from(op_node_sign(
      handle,
      this.hash.digest(),
      this.#digestType,
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
    // deno-lint-ignore no-explicit-any
    publicKey: any,
    signature: BinaryLike,
    encoding?: BinaryToTextEncoding,
  ): boolean {
    const res = prepareAsymmetricKey(publicKey, kConsumePublic);
    let handle;
    if ("handle" in res) {
      handle = res.handle;
    } else {
      handle = op_node_create_public_key(
        res.data,
        res.format,
        res.type ?? "",
        res.passphrase,
      );
    }
    return op_node_verify(
      handle,
      this.hash.digest(),
      this.#digestType,
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

  const res = prepareAsymmetricKey(key, kConsumePrivate);
  let handle;
  if ("handle" in res) {
    handle = res.handle;
  } else {
    handle = op_node_create_private_key(
      res.data,
      res.format,
      res.type ?? "",
      res.passphrase,
    );
  }

  if (algorithm == null) {
    if (op_node_get_asymmetric_key_type(handle) === "ed25519") {
      algorithm = "sha512";
    } else {
      throw new TypeError(
        "Algorithm must be specified when using non-Ed25519 keys",
      );
    }
  }

  const result = Sign(algorithm!).update(data)
    .sign(new PrivateKeyObject(handle));

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

  const res = prepareAsymmetricKey(key, kConsumePublic);
  let handle;
  if ("handle" in res) {
    handle = res.handle;
  } else {
    handle = op_node_create_public_key(
      res.data,
      res.format,
      res.type ?? "",
      res.passphrase,
    );
  }

  if (algorithm == null) {
    if (op_node_get_asymmetric_key_type(handle) === "ed25519") {
      algorithm = "sha512";
    } else {
      throw new TypeError(
        "Algorithm must be specified when using non-Ed25519 keys",
      );
    }
  }

  const result = Verify(algorithm!).update(data)
    .verify(new PublicKeyObject(handle), signature);

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

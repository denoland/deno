// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

import { notImplemented } from "ext:deno_node/_utils.ts";
import { validateString } from "ext:deno_node/internal/validators.mjs";
import { Buffer } from "ext:deno_node/buffer.ts";
import type { WritableOptions } from "ext:deno_node/_stream.d.ts";
import Writable from "ext:deno_node/internal/streams/writable.mjs";
import type {
  BinaryLike,
  BinaryToTextEncoding,
  Encoding,
  PrivateKeyInput,
  PublicKeyInput,
} from "ext:deno_node/internal/crypto/types.ts";
import { KeyObject } from "ext:deno_node/internal/crypto/keys.ts";
import { createHash, Hash } from "ext:deno_node/internal/crypto/hash.ts";
import { KeyFormat, KeyType } from "ext:deno_node/internal/crypto/types.ts";
import { isArrayBufferView } from "ext:deno_node/internal/util/types.ts";

const { core } = globalThis.__bootstrap;
const { ops } = core;

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

export class Sign extends Writable {
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

    if (algorithm.startsWith("rsa-")) {
      // Allows RSA-[digest_algorithm] as a valid algorithm
      algorithm = algorithm.slice(4);
    }
    this.#digestType = algorithm;
    this.hash = createHash(this.#digestType);
  }

  sign(
    privateKey: KeyLike | SignKeyObjectInput | SignPrivateKeyInput,
    encoding?: BinaryToTextEncoding,
  ): Buffer | string {
    let keyData: Uint8Array;
    let keyType: KeyType;
    let keyFormat: KeyFormat;
    if (typeof privateKey === "string" || isArrayBufferView(privateKey)) {
      // if the key is BinaryLike, interpret it as a PEM encoded RSA key
      keyData = privateKey;
      keyType = "rsa";
      keyFormat = "pem";
    } else {
      // TODO(kt3k): Add support for the case when privateKey is a KeyObject,
      // CryptoKey, etc
      notImplemented("crypto.Sign.prototype.sign with non BinaryLike input");
    }
    const ret = Buffer.from(ops.op_node_sign(
      this.hash.digest(),
      this.#digestType,
      keyData!,
      keyType,
      keyFormat,
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

export class Verify extends Writable {
  constructor(algorithm: string, _options?: WritableOptions) {
    validateString(algorithm, "algorithm");

    super();

    notImplemented("crypto.Verify");
  }

  update(data: BinaryLike): this;
  update(data: string, inputEncoding: Encoding): this;
  update(_data: BinaryLike, _inputEncoding?: string): this {
    notImplemented("crypto.Sign.prototype.update");
  }

  verify(
    object: KeyLike | VerifyKeyObjectInput | VerifyPublicKeyInput,
    signature: ArrayBufferView,
  ): boolean;
  verify(
    object: KeyLike | VerifyKeyObjectInput | VerifyPublicKeyInput,
    signature: string,
    signatureEncoding?: BinaryToTextEncoding,
  ): boolean;
  verify(
    _object: KeyLike | VerifyKeyObjectInput | VerifyPublicKeyInput,
    _signature: ArrayBufferView | string,
    _signatureEncoding?: BinaryToTextEncoding,
  ): boolean {
    notImplemented("crypto.Sign.prototype.sign");
  }
}

export function signOneShot(
  algorithm: string | null | undefined,
  data: ArrayBufferView,
  key: KeyLike | SignKeyObjectInput | SignPrivateKeyInput,
): Buffer;
export function signOneShot(
  algorithm: string | null | undefined,
  data: ArrayBufferView,
  key: KeyLike | SignKeyObjectInput | SignPrivateKeyInput,
  callback: (error: Error | null, data: Buffer) => void,
): void;
export function signOneShot(
  _algorithm: string | null | undefined,
  _data: ArrayBufferView,
  _key: KeyLike | SignKeyObjectInput | SignPrivateKeyInput,
  _callback?: (error: Error | null, data: Buffer) => void,
): Buffer | void {
  notImplemented("crypto.sign");
}

export function verifyOneShot(
  algorithm: string | null | undefined,
  data: ArrayBufferView,
  key: KeyLike | VerifyKeyObjectInput | VerifyPublicKeyInput,
  signature: ArrayBufferView,
): boolean;
export function verifyOneShot(
  algorithm: string | null | undefined,
  data: ArrayBufferView,
  key: KeyLike | VerifyKeyObjectInput | VerifyPublicKeyInput,
  signature: ArrayBufferView,
  callback: (error: Error | null, result: boolean) => void,
): void;
export function verifyOneShot(
  _algorithm: string | null | undefined,
  _data: ArrayBufferView,
  _key: KeyLike | VerifyKeyObjectInput | VerifyPublicKeyInput,
  _signature: ArrayBufferView,
  _callback?: (error: Error | null, result: boolean) => void,
): boolean | void {
  notImplemented("crypto.verify");
}

export default {
  signOneShot,
  verifyOneShot,
  Sign,
  Verify,
};

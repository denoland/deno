// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

import { notImplemented } from "internal:deno_node/polyfills/_utils.ts";
import { validateString } from "internal:deno_node/polyfills/internal/validators.mjs";
import { Buffer } from "internal:deno_node/polyfills/buffer.ts";
import type { WritableOptions } from "internal:deno_node/polyfills/_stream.d.ts";
import Writable from "internal:deno_node/polyfills/internal/streams/writable.mjs";
import type {
  BinaryLike,
  BinaryToTextEncoding,
  Encoding,
  PrivateKeyInput,
  PublicKeyInput,
} from "internal:deno_node/polyfills/internal/crypto/types.ts";
import { KeyObject } from "internal:deno_node/polyfills/internal/crypto/keys.ts";

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
  constructor(algorithm: string, _options?: WritableOptions) {
    validateString(algorithm, "algorithm");

    super();

    notImplemented("crypto.Sign");
  }

  sign(privateKey: KeyLike | SignKeyObjectInput | SignPrivateKeyInput): Buffer;
  sign(
    privateKey: KeyLike | SignKeyObjectInput | SignPrivateKeyInput,
    outputFormat: BinaryToTextEncoding,
  ): string;
  sign(
    _privateKey: KeyLike | SignKeyObjectInput | SignPrivateKeyInput,
    _outputEncoding?: BinaryToTextEncoding,
  ): Buffer | string {
    notImplemented("crypto.Sign.prototype.sign");
  }

  update(data: BinaryLike): this;
  update(data: string, inputEncoding: Encoding): this;
  update(_data: BinaryLike | string, _inputEncoding?: Encoding): this {
    notImplemented("crypto.Sign.prototype.update");
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

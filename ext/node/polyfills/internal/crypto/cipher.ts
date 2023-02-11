// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

import { ERR_INVALID_ARG_TYPE } from "internal:deno_node/polyfills/internal/errors.ts";
import {
  validateInt32,
  validateObject,
} from "internal:deno_node/polyfills/internal/validators.mjs";
import { Buffer } from "internal:deno_node/polyfills/buffer.ts";
import { notImplemented } from "internal:deno_node/polyfills/_utils.ts";
import type { TransformOptions } from "internal:deno_node/polyfills/_stream.d.ts";
import { Transform } from "internal:deno_node/polyfills/_stream.mjs";
import { KeyObject } from "internal:deno_node/polyfills/internal/crypto/keys.ts";
import type { BufferEncoding } from "internal:deno_node/polyfills/_global.d.ts";
import type {
  BinaryLike,
  Encoding,
} from "internal:deno_node/polyfills/internal/crypto/types.ts";
import {
  privateDecrypt,
  privateEncrypt,
  publicDecrypt,
  publicEncrypt,
} from "internal:deno_node/polyfills/_crypto/crypto_browserify/public_encrypt/mod.js";

export {
  privateDecrypt,
  privateEncrypt,
  publicDecrypt,
  publicEncrypt,
} from "internal:deno_node/polyfills/_crypto/crypto_browserify/public_encrypt/mod.js";

export type CipherCCMTypes =
  | "aes-128-ccm"
  | "aes-192-ccm"
  | "aes-256-ccm"
  | "chacha20-poly1305";
export type CipherGCMTypes = "aes-128-gcm" | "aes-192-gcm" | "aes-256-gcm";
export type CipherOCBTypes = "aes-128-ocb" | "aes-192-ocb" | "aes-256-ocb";

export type CipherKey = BinaryLike | KeyObject;

export interface CipherCCMOptions extends TransformOptions {
  authTagLength: number;
}

export interface CipherGCMOptions extends TransformOptions {
  authTagLength?: number | undefined;
}

export interface CipherOCBOptions extends TransformOptions {
  authTagLength: number;
}

export interface Cipher extends ReturnType<typeof Transform> {
  update(data: BinaryLike): Buffer;
  update(data: string, inputEncoding: Encoding): Buffer;
  update(
    data: ArrayBufferView,
    inputEncoding: undefined,
    outputEncoding: Encoding,
  ): string;
  update(
    data: string,
    inputEncoding: Encoding | undefined,
    outputEncoding: Encoding,
  ): string;

  final(): Buffer;
  final(outputEncoding: BufferEncoding): string;

  setAutoPadding(autoPadding?: boolean): this;
}

export type Decipher = Cipher;

export interface CipherCCM extends Cipher {
  setAAD(
    buffer: ArrayBufferView,
    options: {
      plaintextLength: number;
    },
  ): this;
  getAuthTag(): Buffer;
}

export interface CipherGCM extends Cipher {
  setAAD(
    buffer: ArrayBufferView,
    options?: {
      plaintextLength: number;
    },
  ): this;
  getAuthTag(): Buffer;
}

export interface CipherOCB extends Cipher {
  setAAD(
    buffer: ArrayBufferView,
    options?: {
      plaintextLength: number;
    },
  ): this;
  getAuthTag(): Buffer;
}

export interface DecipherCCM extends Decipher {
  setAuthTag(buffer: ArrayBufferView): this;
  setAAD(
    buffer: ArrayBufferView,
    options: {
      plaintextLength: number;
    },
  ): this;
}

export interface DecipherGCM extends Decipher {
  setAuthTag(buffer: ArrayBufferView): this;
  setAAD(
    buffer: ArrayBufferView,
    options?: {
      plaintextLength: number;
    },
  ): this;
}

export interface DecipherOCB extends Decipher {
  setAuthTag(buffer: ArrayBufferView): this;
  setAAD(
    buffer: ArrayBufferView,
    options?: {
      plaintextLength: number;
    },
  ): this;
}

export class Cipheriv extends Transform implements Cipher {
  constructor(
    _cipher: string,
    _key: CipherKey,
    _iv: BinaryLike | null,
    _options?: TransformOptions,
  ) {
    super();

    notImplemented("crypto.Cipheriv");
  }

  final(): Buffer;
  final(outputEncoding: BufferEncoding): string;
  final(_outputEncoding?: string): Buffer | string {
    notImplemented("crypto.Cipheriv.prototype.final");
  }

  getAuthTag(): Buffer {
    notImplemented("crypto.Cipheriv.prototype.getAuthTag");
  }

  setAAD(
    _buffer: ArrayBufferView,
    _options?: {
      plaintextLength: number;
    },
  ): this {
    notImplemented("crypto.Cipheriv.prototype.setAAD");
  }

  setAutoPadding(_autoPadding?: boolean): this {
    notImplemented("crypto.Cipheriv.prototype.setAutoPadding");
  }

  update(data: BinaryLike): Buffer;
  update(data: string, inputEncoding: Encoding): Buffer;
  update(
    data: ArrayBufferView,
    inputEncoding: undefined,
    outputEncoding: Encoding,
  ): string;
  update(
    data: string,
    inputEncoding: Encoding | undefined,
    outputEncoding: Encoding,
  ): string;
  update(
    _data: string | BinaryLike | ArrayBufferView,
    _inputEncoding?: Encoding,
    _outputEncoding?: Encoding,
  ): Buffer | string {
    notImplemented("crypto.Cipheriv.prototype.update");
  }
}

export class Decipheriv extends Transform implements Cipher {
  constructor(
    _cipher: string,
    _key: CipherKey,
    _iv: BinaryLike | null,
    _options?: TransformOptions,
  ) {
    super();

    notImplemented("crypto.Decipheriv");
  }

  final(): Buffer;
  final(outputEncoding: BufferEncoding): string;
  final(_outputEncoding?: string): Buffer | string {
    notImplemented("crypto.Decipheriv.prototype.final");
  }

  setAAD(
    _buffer: ArrayBufferView,
    _options?: {
      plaintextLength: number;
    },
  ): this {
    notImplemented("crypto.Decipheriv.prototype.setAAD");
  }

  setAuthTag(_buffer: BinaryLike, _encoding?: string): this {
    notImplemented("crypto.Decipheriv.prototype.setAuthTag");
  }

  setAutoPadding(_autoPadding?: boolean): this {
    notImplemented("crypto.Decipheriv.prototype.setAutoPadding");
  }

  update(data: BinaryLike): Buffer;
  update(data: string, inputEncoding: Encoding): Buffer;
  update(
    data: ArrayBufferView,
    inputEncoding: undefined,
    outputEncoding: Encoding,
  ): string;
  update(
    data: string,
    inputEncoding: Encoding | undefined,
    outputEncoding: Encoding,
  ): string;
  update(
    _data: string | BinaryLike | ArrayBufferView,
    _inputEncoding?: Encoding,
    _outputEncoding?: Encoding,
  ): Buffer | string {
    notImplemented("crypto.Decipheriv.prototype.update");
  }
}

export function getCipherInfo(
  nameOrNid: string | number,
  options?: { keyLength?: number; ivLength?: number },
) {
  if (typeof nameOrNid !== "string" && typeof nameOrNid !== "number") {
    throw new ERR_INVALID_ARG_TYPE(
      "nameOrNid",
      ["string", "number"],
      nameOrNid,
    );
  }

  if (typeof nameOrNid === "number") {
    validateInt32(nameOrNid, "nameOrNid");
  }

  let keyLength, ivLength;

  if (options !== undefined) {
    validateObject(options, "options");

    ({ keyLength, ivLength } = options);

    if (keyLength !== undefined) {
      validateInt32(keyLength, "options.keyLength");
    }

    if (ivLength !== undefined) {
      validateInt32(ivLength, "options.ivLength");
    }
  }

  notImplemented("crypto.getCipherInfo");
}

export default {
  privateDecrypt,
  privateEncrypt,
  publicDecrypt,
  publicEncrypt,
  Cipheriv,
  Decipheriv,
  getCipherInfo,
};

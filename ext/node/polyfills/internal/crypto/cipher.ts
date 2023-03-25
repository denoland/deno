// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

import { ERR_INVALID_ARG_TYPE } from "ext:deno_node/internal/errors.ts";
import {
  validateInt32,
  validateObject,
} from "ext:deno_node/internal/validators.mjs";
import { Buffer } from "ext:deno_node/buffer.ts";
import { notImplemented } from "ext:deno_node/_utils.ts";
import type { TransformOptions } from "ext:deno_node/_stream.d.ts";
import { Transform } from "ext:deno_node/_stream.mjs";
import { KeyObject } from "./keys.ts";
import type { BufferEncoding } from "ext:deno_node/_global.d.ts";
import type {
  BinaryLike,
  Encoding,
} from "ext:deno_node/internal/crypto/types.ts";
import { getDefaultEncoding } from "ext:deno_node/internal/crypto/util.ts";

const { ops, encode } = globalThis.__bootstrap.core;

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
  update(
    data: string,
    inputEncoding?: Encoding,
    outputEncoding?: Encoding,
  ): string;

  final(outputEncoding?: BufferEncoding): string;

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

function toU8(input: string | Uint8Array): Uint8Array {
  return typeof input === "string" ? encode(input) : input;
}

export class Cipheriv extends Transform implements Cipher {
  /** CipherContext resource id */
  #context: number;

  /** plaintext data cache */
  #cache: BlockModeCache;

  constructor(
    cipher: string,
    key: CipherKey,
    iv: BinaryLike | null,
    options?: TransformOptions,
  ) {
    super({
      transform(chunk, encoding, cb) {
        this.push(this.update(chunk, encoding));
        cb();
      },
      final(cb) {
        this.push(this.final());
        cb();
      },
      ...options,
    });
    this.#cache = new BlockModeCache(false);
    this.#context = ops.op_node_create_cipheriv(cipher, toU8(key), toU8(iv));
  }

  final(encoding: string = getDefaultEncoding()): Buffer | string {
    const buf = new Buffer(16);
    ops.op_node_cipheriv_final(this.#context, this.#cache.cache, buf);
    return encoding === "buffer" ? buf : buf.toString(encoding);
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
    return this;
  }

  setAutoPadding(_autoPadding?: boolean): this {
    notImplemented("crypto.Cipheriv.prototype.setAutoPadding");
    return this;
  }

  update(
    data: string | Buffer | ArrayBufferView,
    inputEncoding?: Encoding,
    outputEncoding: Encoding = getDefaultEncoding(),
  ): Buffer | string {
    // TODO(kt3k): throw ERR_INVALID_ARG_TYPE if data is not string, Buffer, or ArrayBufferView
    if (typeof data === "string" && typeof inputEncoding === "string") {
      this.#cache.add(Buffer.from(data, inputEncoding));
    } else {
      this.#cache.add(data);
    }
    const input = this.#cache.get();
    let output;
    if (input === null) {
      output = Buffer.alloc(0);
    } else {
      output = Buffer.allocUnsafe(input.length);
      ops.op_node_cipheriv_encrypt(this.#context, input, output);
    }
    return outputEncoding === "buffer"
      ? output
      : output.toString(outputEncoding);
  }
}

/** Caches data and output the chunk of multiple of 16.
 * Used by CBC, ECB modes of block ciphers */
class BlockModeCache {
  cache: Uint8Array;
  // The last chunk can be padded when decrypting.
  #lastChunkIsNonZero: boolean;

  constructor(lastChunkIsNotZero = false) {
    this.cache = new Uint8Array(0);
    this.#lastChunkIsNonZero = lastChunkIsNotZero;
  }

  add(data: Uint8Array) {
    const cache = this.cache;
    this.cache = new Uint8Array(cache.length + data.length);
    this.cache.set(cache);
    this.cache.set(data, cache.length);
  }

  /** Gets the chunk of the length of largest multiple of 16.
   * Used for preparing data for encryption/decryption */
  get(): Uint8Array | null {
    let len = this.cache.length;
    if (this.#lastChunkIsNonZero) {
      // Reduces the available chunk length by 1 to keep the last chunk
      len -= 1;
    }
    if (len < 16) {
      return null;
    }

    len = Math.floor(len / 16) * 16;
    const out = this.cache.subarray(0, len);
    this.cache = this.cache.subarray(len);
    return out;
  }
}

export class Decipheriv extends Transform implements Cipher {
  /** DecipherContext resource id */
  #context: number;

  /** ciphertext data cache */
  #cache: BlockModeCache;

  constructor(
    cipher: string,
    key: CipherKey,
    iv: BinaryLike | null,
    options?: TransformOptions,
  ) {
    super({
      transform(chunk, encoding, cb) {
        this.push(this.update(chunk, encoding));
        cb();
      },
      final(cb) {
        this.push(this.final());
        cb();
      },
      ...options,
    });
    this.#cache = new BlockModeCache(true);
    this.#context = ops.op_node_create_decipheriv(cipher, toU8(key), toU8(iv));
  }

  final(encoding: string = getDefaultEncoding()): Buffer | string {
    let buf = new Buffer(16);
    ops.op_node_decipheriv_final(this.#context, this.#cache.cache, buf);
    buf = buf.subarray(0, 16 - buf.at(-1)); // Padded in Pkcs7 mode
    return encoding === "buffer" ? buf : buf.toString(encoding);
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

  update(
    data: string | Buffer | ArrayBufferView,
    inputEncoding?: Encoding,
    outputEncoding: Encoding = getDefaultEncoding(),
  ): Buffer | string {
    // TODO(kt3k): throw ERR_INVALID_ARG_TYPE if data is not string, Buffer, or ArrayBufferView
    if (typeof data === "string" && typeof inputEncoding === "string") {
      this.#cache.add(Buffer.from(data, inputEncoding));
    } else {
      this.#cache.add(data);
    }
    const input = this.#cache.get();
    let output;
    if (input === null) {
      output = Buffer.alloc(0);
    } else {
      output = new Buffer(input.length);
      ops.op_node_decipheriv_decrypt(this.#context, input, output);
    }
    return outputEncoding === "buffer"
      ? output
      : output.toString(outputEncoding);
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

export function privateEncrypt(
  privateKey: ArrayBufferView | string | KeyObject,
  buffer: ArrayBufferView | string | KeyObject,
): Buffer {
  const padding = privateKey.padding || 1;
  return ops.op_node_private_encrypt(privateKey, buffer, padding);
}

export function privateDecrypt(
  privateKey: ArrayBufferView | string | KeyObject,
  buffer: ArrayBufferView | string | KeyObject,
): Buffer {
  const padding = privateKey.padding || 1;
  return ops.op_node_private_decrypt(privateKey, buffer, padding);
}

export function publicEncrypt(
  publicKey: ArrayBufferView | string | KeyObject,
  buffer: ArrayBufferView | string | KeyObject,
): Buffer {
  const padding = publicKey.padding || 1;
  return ops.op_node_public_encrypt(publicKey, buffer, padding);
}

export function publicDecrypt() {
  notImplemented("crypto.publicDecrypt");
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

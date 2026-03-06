// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials no-explicit-any

import { core, primordials } from "ext:core/mod.js";
const {
  encode,
} = core;
const {
  SymbolSpecies,
} = primordials;
import {
  op_node_cipheriv_encrypt,
  op_node_cipheriv_final,
  op_node_cipheriv_set_aad,
  op_node_cipheriv_take,
  op_node_create_cipheriv,
  op_node_create_decipheriv,
  op_node_decipheriv_auth_tag,
  op_node_decipheriv_decrypt,
  op_node_decipheriv_final,
  op_node_decipheriv_set_aad,
  op_node_export_secret_key,
  op_node_private_decrypt,
  op_node_private_encrypt,
  op_node_public_encrypt,
} from "ext:core/ops";

import { Buffer } from "node:buffer";
import { notImplemented } from "ext:deno_node/_utils.ts";
import type { TransformOptions } from "ext:deno_node/_stream.d.ts";
import { Transform } from "node:stream";
import {
  getArrayBufferOrView,
  KeyObject,
} from "ext:deno_node/internal/crypto/keys.ts";
import { isKeyObject } from "ext:deno_node/internal/crypto/_keys.ts";
import { kHandle } from "ext:deno_node/internal/crypto/constants.ts";
import type { BufferEncoding } from "ext:deno_node/_global.d.ts";
import type {
  BinaryLike,
  Encoding,
} from "ext:deno_node/internal/crypto/types.ts";
import { getDefaultEncoding } from "ext:deno_node/internal/crypto/util.ts";
import {
  ERR_INVALID_ARG_VALUE,
  ERR_UNKNOWN_ENCODING,
} from "ext:deno_node/internal/errors.ts";

import {
  isAnyArrayBuffer,
  isArrayBufferView,
} from "ext:deno_node/internal/util/types.ts";
import { ERR_CRYPTO_INVALID_STATE } from "ext:deno_node/internal/errors.ts";
import { StringDecoder } from "node:string_decoder";
import assert from "node:assert";
import { normalizeEncoding } from "ext:deno_node/internal/util.mjs";

const FastBuffer = Buffer[SymbolSpecies];

export function isStringOrBuffer(
  val: unknown,
): val is string | Buffer | ArrayBuffer | ArrayBufferView {
  return typeof val === "string" ||
    isArrayBufferView(val) ||
    isAnyArrayBuffer(val) ||
    Buffer.isBuffer(val);
}

const NO_TAG = new Uint8Array();

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

function toU8(
  input: string | Uint8Array | KeyObject | null,
): Uint8Array {
  if (input == null) {
    return new Uint8Array(0);
  }
  if (isKeyObject(input)) {
    return op_node_export_secret_key(input[kHandle]);
  }
  return typeof input === "string" ? encode(input) : input;
}

export function Cipheriv(
  cipher: string,
  key: CipherKey,
  iv: BinaryLike | null,
  options?: TransformOptions,
) {
  if (!(this instanceof Cipheriv)) {
    return new Cipheriv(cipher, key, iv, options);
  }

  const authTagLength = getUIntOption(options, "authTagLength");

  Transform.call(this, {
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

  this._blockSize = getBlockSize(cipher);
  this._cache = new BlockModeCache(false, this._blockSize);
  this._context = op_node_create_cipheriv(
    cipher,
    toU8(key),
    toU8(iv),
    authTagLength,
  );
  this._needsBlockCache =
    !(cipher == "aes-128-gcm" || cipher == "aes-256-gcm" ||
      cipher == "aes-128-ctr" || cipher == "aes-192-ctr" ||
      cipher == "aes-256-ctr");
  this._authTag = undefined;
  this._autoPadding = true;
  this._finalized = false;
  this._decoder = undefined;

  if (this._context == 0) {
    throw new TypeError("Unknown cipher");
  }
}

Object.setPrototypeOf(Cipheriv.prototype, Transform.prototype);
Object.setPrototypeOf(Cipheriv, Transform);

Cipheriv.prototype.final = function (
  encoding: string = getDefaultEncoding(),
): Buffer | string {
  if (this._finalized) {
    throw new ERR_CRYPTO_INVALID_STATE("final");
  }

  _lazyInitCipherDecoder(this, encoding);

  const bs = this._blockSize;
  const buf = new FastBuffer(bs);
  const hasNoBufferedData = this._cache.cache.byteLength === 0;
  const shouldPadEmptyBlock = this._needsBlockCache && this._autoPadding;

  if (hasNoBufferedData && !shouldPadEmptyBlock) {
    const maybeTag = op_node_cipheriv_take(this._context);
    if (maybeTag) this._authTag = Buffer.from(maybeTag);
    this._finalized = true;
    return encoding === "buffer" ? Buffer.from([]) : "";
  }

  if (!this._autoPadding && this._cache.cache.byteLength != bs) {
    throw new Error("Invalid final block size");
  }
  const maybeTag = op_node_cipheriv_final(
    this._context,
    this._autoPadding,
    this._cache.cache,
    buf,
  );
  if (maybeTag) {
    this._authTag = Buffer.from(maybeTag);
    this._finalized = true;
    return encoding === "buffer" ? Buffer.from([]) : "";
  }

  this._finalized = true;
  if (encoding !== "buffer") {
    return this._decoder!.end(buf);
  }

  return buf;
};

Cipheriv.prototype.getAuthTag = function (): Buffer {
  if (!this._authTag) {
    throw new ERR_CRYPTO_INVALID_STATE("getAuthTag");
  }
  return this._authTag;
};

Cipheriv.prototype.setAAD = function (
  buffer: ArrayBufferView,
  _options?: {
    plaintextLength: number;
  },
) {
  op_node_cipheriv_set_aad(this._context, buffer);
  return this;
};

Cipheriv.prototype.setAutoPadding = function (autoPadding?: boolean) {
  this._autoPadding = !!autoPadding;
  return this;
};

Cipheriv.prototype.update = function (
  data: string | Buffer | ArrayBufferView,
  inputEncoding?: Encoding,
  outputEncoding: Encoding = getDefaultEncoding(),
): Buffer | string {
  if (this._finalized) {
    throw new ERR_CRYPTO_INVALID_STATE("update");
  }

  // TODO(kt3k): throw ERR_INVALID_ARG_TYPE if data is not string, Buffer, or ArrayBufferView
  let buf = data;
  if (typeof data === "string") {
    buf = Buffer.from(data, inputEncoding);
  }
  _lazyInitCipherDecoder(this, outputEncoding);

  let output: Buffer;
  if (!this._needsBlockCache) {
    output = Buffer.allocUnsafe(buf.length);
    op_node_cipheriv_encrypt(this._context, buf, output);

    if (outputEncoding !== "buffer") {
      return this._decoder!.write(output);
    }

    return output;
  }

  this._cache.add(buf);
  const input = this._cache.get();

  if (input === null) {
    output = Buffer.alloc(0);
  } else {
    output = Buffer.allocUnsafe(input.length);
    op_node_cipheriv_encrypt(this._context, input, output);
  }

  if (outputEncoding !== "buffer") {
    return this._decoder!.write(output);
  }

  return output;
};

function _lazyInitCipherDecoder(self: any, encoding: string) {
  if (encoding === "buffer") {
    return;
  }

  const normalizedEncoding = normalizeEncoding(encoding);
  self._decoder ||= new StringDecoder(normalizedEncoding);

  if (self._decoder.encoding !== normalizedEncoding) {
    if (normalizedEncoding === undefined) {
      throw new ERR_UNKNOWN_ENCODING(encoding);
    }
    assert(false, "Cannot change encoding");
  }
}

/** Caches data and output the chunk of multiple of 16.
 * Used by CBC, ECB modes of block ciphers */
class BlockModeCache {
  cache: Uint8Array;
  blockSize: number;
  // The last chunk can be padded when decrypting.
  #lastChunkIsNonZero: boolean;

  constructor(lastChunkIsNotZero = false, blockSize = 16) {
    this.cache = new Uint8Array(0);
    this.blockSize = blockSize;
    this.#lastChunkIsNonZero = lastChunkIsNotZero;
  }

  add(data: Uint8Array) {
    const cache = this.cache;
    this.cache = new Uint8Array(cache.length + data.length);
    this.cache.set(cache);
    this.cache.set(data, cache.length);
  }

  /** Gets the chunk of the length of largest multiple of blockSize.
   * Used for preparing data for encryption/decryption */
  get(): Uint8Array | null {
    const bs = this.blockSize;
    let len = this.cache.length;
    if (this.#lastChunkIsNonZero) {
      // Reduces the available chunk length by 1 to keep the last chunk
      len -= 1;
    }
    if (len < bs) {
      return null;
    }

    len = Math.floor(len / bs) * bs;
    const out = this.cache.subarray(0, len);
    this.cache = this.cache.subarray(len);
    return out;
  }

  set lastChunkIsNonZero(value: boolean) {
    this.#lastChunkIsNonZero = value;
  }
}

function getBlockSize(cipher: string): number {
  if (cipher.startsWith("des")) {
    return 8;
  }
  return 16;
}

function getUIntOption(options, key) {
  let value;
  if (options && (value = options[key]) != null) {
    if (value >>> 0 !== value) {
      throw new ERR_INVALID_ARG_VALUE(`options.${key}`, value);
    }
    return value;
  }
  return -1;
}

export function Decipheriv(
  cipher: string,
  key: CipherKey,
  iv: BinaryLike | null,
  options?: TransformOptions,
) {
  if (!(this instanceof Decipheriv)) {
    return new Decipheriv(cipher, key, iv, options);
  }

  const authTagLength = getUIntOption(options, "authTagLength");

  Transform.call(this, {
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

  this._autoPadding = true;
  this._blockSize = getBlockSize(cipher);
  this._cache = new BlockModeCache(this._autoPadding, this._blockSize);
  this._context = op_node_create_decipheriv(
    cipher,
    toU8(key),
    toU8(iv),
    authTagLength,
  );
  this._needsBlockCache =
    !(cipher == "aes-128-gcm" || cipher == "aes-256-gcm" ||
      cipher == "aes-128-ctr" || cipher == "aes-192-ctr" ||
      cipher == "aes-256-ctr");
  this._authTag = undefined;
  this._finalized = false;
  this._decoder = undefined;

  if (this._context == 0) {
    throw new TypeError("Unknown cipher");
  }
}

Object.setPrototypeOf(Decipheriv.prototype, Transform.prototype);
Object.setPrototypeOf(Decipheriv, Transform);

Decipheriv.prototype.final = function (
  encoding: string = getDefaultEncoding(),
): Buffer | string {
  if (this._finalized) {
    throw new ERR_CRYPTO_INVALID_STATE("final");
  }

  _lazyInitDecipherDecoder(this, encoding);

  const bs = this._blockSize;
  let buf = new FastBuffer(bs);
  op_node_decipheriv_final(
    this._context,
    this._autoPadding,
    this._cache.cache,
    buf,
    this._authTag || NO_TAG,
  );

  if (!this._needsBlockCache || this._cache.cache.byteLength === 0) {
    this._finalized = true;
    return encoding === "buffer" ? Buffer.from([]) : "";
  }
  if (this._cache.cache.byteLength != bs) {
    throw new Error("Invalid final block size");
  }

  if (this._autoPadding) {
    const padLen = buf.at(-1);
    if (padLen === 0 || padLen > bs) {
      throw new Error("bad decrypt");
    }
    buf = buf.subarray(0, bs - padLen); // Padded in Pkcs7 mode
  }
  this._finalized = true;
  if (encoding !== "buffer") {
    return this._decoder!.end(buf);
  }

  return buf;
};

Decipheriv.prototype.setAAD = function (
  buffer: ArrayBufferView,
  _options?: {
    plaintextLength: number;
  },
) {
  op_node_decipheriv_set_aad(this._context, buffer);
  return this;
};

Decipheriv.prototype.setAuthTag = function (
  buffer: BinaryLike,
  _encoding?: string,
) {
  op_node_decipheriv_auth_tag(this._context, buffer.byteLength);
  this._authTag = buffer;
  return this;
};

Decipheriv.prototype.setAutoPadding = function (autoPadding?: boolean) {
  this._autoPadding = Boolean(autoPadding);
  this._cache.lastChunkIsNonZero = this._autoPadding;
  return this;
};

Decipheriv.prototype.update = function (
  data: string | Buffer | ArrayBufferView,
  inputEncoding?: Encoding,
  outputEncoding: Encoding = getDefaultEncoding(),
): Buffer | string {
  if (this._finalized) {
    throw new ERR_CRYPTO_INVALID_STATE("update");
  }

  // TODO(kt3k): throw ERR_INVALID_ARG_TYPE if data is not string, Buffer, or ArrayBufferView
  let buf = data;
  if (typeof data === "string") {
    buf = Buffer.from(data, inputEncoding);
  }
  _lazyInitDecipherDecoder(this, outputEncoding);

  let output;
  if (!this._needsBlockCache) {
    output = Buffer.allocUnsafe(buf.length);
    op_node_decipheriv_decrypt(this._context, buf, output);

    if (outputEncoding !== "buffer") {
      return this._decoder!.write(output);
    }

    return output;
  }

  this._cache.add(buf);
  const input = this._cache.get();
  if (input === null) {
    output = Buffer.alloc(0);
  } else {
    output = new FastBuffer(input.length);
    op_node_decipheriv_decrypt(this._context, input, output);
  }

  if (outputEncoding !== "buffer") {
    return this._decoder!.write(output);
  }

  return output;
};

function _lazyInitDecipherDecoder(self: any, encoding: string) {
  if (encoding === "buffer") {
    return;
  }

  const normalizedEncoding = normalizeEncoding(encoding);
  self._decoder ||= new StringDecoder(normalizedEncoding);

  if (self._decoder.encoding !== normalizedEncoding) {
    if (normalizedEncoding === undefined) {
      throw new ERR_UNKNOWN_ENCODING(encoding);
    }
    assert(false, "Cannot change encoding");
  }
}

export function privateEncrypt(
  privateKey: ArrayBufferView | string | KeyObject,
  buffer: ArrayBufferView | string | KeyObject,
): Buffer {
  const { data } = prepareKey(privateKey);
  const padding = privateKey.padding || 1;

  buffer = getArrayBufferOrView(buffer, "buffer");
  return Buffer.from(op_node_private_encrypt(data, buffer, padding));
}

export function privateDecrypt(
  privateKey: ArrayBufferView | string | KeyObject,
  buffer: ArrayBufferView | string | KeyObject,
): Buffer {
  const { data } = prepareKey(privateKey);
  const padding = privateKey.padding || 1;

  buffer = getArrayBufferOrView(buffer, "buffer");
  return Buffer.from(op_node_private_decrypt(data, buffer, padding));
}

export function publicEncrypt(
  publicKey: ArrayBufferView | string | KeyObject,
  buffer: ArrayBufferView | string | KeyObject,
): Buffer {
  const { data } = prepareKey(publicKey);
  const padding = publicKey.padding || 1;

  buffer = getArrayBufferOrView(buffer, "buffer");
  return Buffer.from(op_node_public_encrypt(data, buffer, padding));
}

export function prepareKey(key) {
  // TODO(@littledivy): handle these cases
  // - web CryptoKey
  if (isStringOrBuffer(key)) {
    return { data: getArrayBufferOrView(key, "key") };
  } else if (isKeyObject(key) && key.type === "public") {
    const data = key.export({ type: "spki", format: "pem" });
    return { data: getArrayBufferOrView(data, "key") };
  } else if (isKeyObject(key) && key.type === "private") {
    const data = key.export({ type: "pkcs8", format: "pem" });
    return { data: getArrayBufferOrView(data, "key") };
  } else if (typeof key == "object") {
    const { key: data, encoding } = key;
    if (!isStringOrBuffer(data)) {
      throw new TypeError("Invalid key type");
    }

    return { data: getArrayBufferOrView(data, "key", encoding) };
  }

  throw new TypeError("Invalid key type");
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
  prepareKey,
};

// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// deno-lint-ignore-file no-explicit-any

(function () {
const { core, primordials } = __bootstrap;
const {
  encode,
} = core;
const {
  ArrayBufferIsView,
  Boolean,
  Error,
  FunctionPrototypeCall,
  MathFloor,
  ObjectPrototypeIsPrototypeOf,
  ObjectSetPrototypeOf,
  SafeRegExp,
  SafeSet,
  SetPrototypeHas,
  StringPrototypeReplace,
  StringPrototypeStartsWith,
  StringPrototypeToLowerCase,
  SymbolSpecies,
  TypeError,
  TypeErrorPrototype,
  TypedArrayPrototypeAt,
  TypedArrayPrototypeGetByteLength,
  Uint8Array,
} = primordials;
const {
  op_node_aes_unwrap_key,
  op_node_aes_wrap_key,
  op_node_cipheriv_encrypt,
  op_node_cipheriv_final,
  op_node_cipheriv_set_aad,
  op_node_cipheriv_take,
  op_node_create_cipheriv,
  op_node_create_decipheriv,
  op_node_create_private_key,
  op_node_decipheriv_auth_tag,
  op_node_decipheriv_decrypt,
  op_node_decipheriv_final,
  op_node_decipheriv_set_aad,
  op_node_export_private_key_pem,
  op_node_export_secret_key,
  op_node_private_decrypt,
  op_node_private_encrypt,
  op_node_public_decrypt,
  op_node_public_encrypt,
  op_node_validate_oaep_hash,
} = core.ops;

const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");

const lazyStream = core.createLazyLoader("node:stream");

const {
  createPrivateKey,
  createPublicKey,
  getArrayBufferOrView,
} = core.loadExtScript("ext:deno_node/internal/crypto/keys.ts");
const { isKeyObject } = core.loadExtScript(
  "ext:deno_node/internal/crypto/_keys.ts",
);
const { kHandle } = core.loadExtScript(
  "ext:deno_node/internal/crypto/constants.ts",
);
const { getDefaultEncoding } = core.loadExtScript(
  "ext:deno_node/internal/crypto/util.ts",
);
const {
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_ARG_VALUE,
  ERR_UNKNOWN_ENCODING,
  NodeError,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");

const {
  isAnyArrayBuffer,
  isArrayBufferView,
} = core.loadExtScript("ext:deno_node/internal/util/types.ts");
const { ERR_CRYPTO_INVALID_STATE, ERR_CRYPTO_UNKNOWN_CIPHER } = core
  .loadExtScript(
    "ext:deno_node/internal/errors.ts",
  );
const { StringDecoder } = core.loadExtScript(
  "ext:deno_node/string_decoder.ts",
);
const { default: assert } = core.loadExtScript("ext:deno_node/assert.ts");
const { normalizeEncoding } = core.loadExtScript(
  "ext:deno_node/internal/util.mjs",
);

let Transform;
function getTransform() {
  if (!Transform) Transform = lazyStream().Transform;
  return Transform;
}

const FastBuffer = Buffer[SymbolSpecies];

function opensslError(code: string, reason: string): NodeError {
  const err = new NodeError(code, reason);
  (err as any).reason = reason;
  return err;
}

function isAesWrap(cipher: string): boolean {
  return cipher === "aes128-wrap" || cipher === "aes192-wrap" ||
    cipher === "aes256-wrap" || cipher === "id-aes128-wrap-pad" ||
    cipher === "id-aes192-wrap-pad" || cipher === "id-aes256-wrap-pad";
}

function isStringOrBuffer(
  val: unknown,
): val is string | Buffer | ArrayBuffer | ArrayBufferView {
  return typeof val === "string" ||
    isArrayBufferView(val) ||
    isAnyArrayBuffer(val) ||
    Buffer.isBuffer(val);
}

// Matches Node's `ArrayBuffer.isView(data)` check in
// `lib/internal/crypto/cipher.js`: accepts string, Buffer, TypedArray
// or DataView, but rejects raw ArrayBuffer / SharedArrayBuffer.
function validateCipherUpdateData(data: unknown): void {
  if (typeof data !== "string" && !ArrayBufferIsView(data)) {
    throw new ERR_INVALID_ARG_TYPE(
      "data",
      ["string", "Buffer", "TypedArray", "DataView"],
      data,
    );
  }
}

const NO_TAG = new Uint8Array();

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

function Cipheriv(
  cipher: string,
  key: any,
  iv: any,
  options?: any,
) {
  if (!ObjectPrototypeIsPrototypeOf(Cipheriv.prototype, this)) {
    return new Cipheriv(cipher, key, iv, options);
  }

  const authTagLength = getUIntOption(options, "authTagLength");

  FunctionPrototypeCall(getTransform(), this, {
    transform(chunk, encoding, cb) {
      // deno-lint-ignore prefer-primordials -- `this` is a Transform stream
      this.push(this.update(chunk, encoding));
      cb();
    },
    final(cb) {
      // deno-lint-ignore prefer-primordials -- `this` is a Transform stream
      this.push(this.final());
      cb();
    },
    ...options,
  });

  this._blockSize = getBlockSize(cipher);
  this._cache = new BlockModeCache(false, this._blockSize);
  this._isAesWrap = isAesWrap(cipher);

  if (this._isAesWrap) {
    this._aesWrapAlgorithm = cipher;
    this._aesWrapKey = toU8(key);
    this._aesWrapIv = toU8(iv);
    this._context = 1; // non-zero sentinel; not used for wrap ops
  } else {
    try {
      this._context = op_node_create_cipheriv(
        cipher,
        toU8(key),
        toU8(iv),
        authTagLength,
      );
    } catch (e) {
      // The op reports an unrecognized algorithm as a TypeError that includes
      // the cipher name; surface Node's ERR_CRYPTO_UNKNOWN_CIPHER instead.
      if (
        ObjectPrototypeIsPrototypeOf(TypeErrorPrototype, e) &&
        StringPrototypeStartsWith(e.message, "Unknown cipher")
      ) {
        throw new ERR_CRYPTO_UNKNOWN_CIPHER();
      }
      throw e;
    }
    if (this._context == 0) {
      throw new ERR_CRYPTO_UNKNOWN_CIPHER();
    }
  }

  this._needsBlockCache = !this._isAesWrap &&
    !(cipher == "aes-128-gcm" || cipher == "aes-256-gcm" ||
      cipher == "aes-128-ctr" || cipher == "aes-192-ctr" ||
      cipher == "aes-256-ctr" || cipher == "chacha20-poly1305");
  this._authTag = undefined;
  this._autoPadding = true;
  this._finalized = false;
  this._decoder = undefined;
}

ObjectSetPrototypeOf(Cipheriv.prototype, getTransform().prototype);
ObjectSetPrototypeOf(Cipheriv, getTransform());

Cipheriv.prototype.final = function (
  encoding: string = getDefaultEncoding(),
): Buffer | string {
  if (this._finalized) {
    throw new ERR_CRYPTO_INVALID_STATE("final");
  }

  if (this._isAesWrap) {
    this._finalized = true;
    return encoding === "buffer" ? Buffer.from([]) : "";
  }

  _lazyInitCipherDecoder(this, encoding);

  const bs = this._blockSize;
  const buf = new FastBuffer(bs);
  const hasNoBufferedData =
    TypedArrayPrototypeGetByteLength(this._cache.cache) === 0;
  const shouldPadEmptyBlock = this._needsBlockCache && this._autoPadding;

  if (hasNoBufferedData && !shouldPadEmptyBlock) {
    const maybeTag = op_node_cipheriv_take(this._context);
    if (maybeTag) this._authTag = Buffer.from(maybeTag);
    this._finalized = true;
    return encoding === "buffer" ? Buffer.from([]) : "";
  }

  if (
    !this._autoPadding &&
    TypedArrayPrototypeGetByteLength(this._cache.cache) != bs
  ) {
    throw opensslError(
      "ERR_OSSL_EVP_WRONG_FINAL_BLOCK_LENGTH",
      "wrong final block length",
    );
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
  if (this._finalized) {
    throw new ERR_CRYPTO_INVALID_STATE("setAAD");
  }
  op_node_cipheriv_set_aad(this._context, buffer);
  return this;
};

Cipheriv.prototype.setAutoPadding = function (autoPadding?: boolean) {
  this._autoPadding = !!autoPadding;
  return this;
};

Cipheriv.prototype.update = function (
  data: string | Buffer | ArrayBufferView,
  inputEncoding?: any,
  outputEncoding: any = getDefaultEncoding(),
): Buffer | string {
  if (this._finalized) {
    throw new ERR_CRYPTO_INVALID_STATE("update");
  }

  validateCipherUpdateData(data);

  let buf = data;
  if (typeof data === "string") {
    buf = Buffer.from(data, inputEncoding);
  }

  // Match Node.js/OpenSSL behavior: reject inputs >= INT_MAX bytes
  if (buf.length >= 2 ** 31 - 1) {
    throw new Error("Trying to add data in unsupported state");
  }

  _lazyInitCipherDecoder(this, outputEncoding);

  if (this._isAesWrap) {
    const output = Buffer.from(
      op_node_aes_wrap_key(
        this._aesWrapAlgorithm,
        this._aesWrapKey,
        this._aesWrapIv,
        buf,
      ),
    );
    if (outputEncoding !== "buffer") {
      return this._decoder!.write(output);
    }
    return output;
  }

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

    len = MathFloor(len / bs) * bs;
    const out = this.cache.subarray(0, len);
    this.cache = this.cache.subarray(len);
    return out;
  }

  set lastChunkIsNonZero(value: boolean) {
    this.#lastChunkIsNonZero = value;
  }
}

function getBlockSize(cipher: string): number {
  if (StringPrototypeStartsWith(cipher, "des")) {
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

function Decipheriv(
  cipher: string,
  key: any,
  iv: any,
  options?: any,
) {
  if (!ObjectPrototypeIsPrototypeOf(Decipheriv.prototype, this)) {
    return new Decipheriv(cipher, key, iv, options);
  }

  const authTagLength = getUIntOption(options, "authTagLength");

  FunctionPrototypeCall(getTransform(), this, {
    transform(chunk, encoding, cb) {
      // deno-lint-ignore prefer-primordials -- `this` is a Transform stream
      this.push(this.update(chunk, encoding));
      cb();
    },
    final(cb) {
      // deno-lint-ignore prefer-primordials -- `this` is a Transform stream
      this.push(this.final());
      cb();
    },
    ...options,
  });

  this._autoPadding = true;
  this._blockSize = getBlockSize(cipher);
  this._cache = new BlockModeCache(this._autoPadding, this._blockSize);
  this._isAesWrap = isAesWrap(cipher);

  if (this._isAesWrap) {
    this._aesWrapAlgorithm = cipher;
    this._aesWrapKey = toU8(key);
    this._aesWrapIv = toU8(iv);
    this._context = 1; // non-zero sentinel; not used for wrap ops
  } else {
    try {
      this._context = op_node_create_decipheriv(
        cipher,
        toU8(key),
        toU8(iv),
        authTagLength,
      );
    } catch (e) {
      // The op reports an unrecognized algorithm as a TypeError that includes
      // the cipher name; surface Node's ERR_CRYPTO_UNKNOWN_CIPHER instead.
      if (
        ObjectPrototypeIsPrototypeOf(TypeErrorPrototype, e) &&
        StringPrototypeStartsWith(e.message, "Unknown cipher")
      ) {
        throw new ERR_CRYPTO_UNKNOWN_CIPHER();
      }
      throw e;
    }
    if (this._context == 0) {
      throw new ERR_CRYPTO_UNKNOWN_CIPHER();
    }
  }

  this._needsBlockCache = !this._isAesWrap &&
    !(cipher == "aes-128-gcm" || cipher == "aes-256-gcm" ||
      cipher == "aes-128-ctr" || cipher == "aes-192-ctr" ||
      cipher == "aes-256-ctr" || cipher == "chacha20-poly1305");
  this._isGcmMode = cipher == "aes-128-gcm" || cipher == "aes-192-gcm" ||
    cipher == "aes-256-gcm";
  this._authTagLength = authTagLength;
  this._authTag = undefined;
  this._finalized = false;
  this._decoder = undefined;
}

ObjectSetPrototypeOf(Decipheriv.prototype, getTransform().prototype);
ObjectSetPrototypeOf(Decipheriv, getTransform());

Decipheriv.prototype.final = function (
  encoding: string = getDefaultEncoding(),
): Buffer | string {
  if (this._finalized) {
    throw new ERR_CRYPTO_INVALID_STATE("final");
  }

  if (this._isAesWrap) {
    this._finalized = true;
    return encoding === "buffer" ? Buffer.from([]) : "";
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

  if (
    !this._needsBlockCache ||
    TypedArrayPrototypeGetByteLength(this._cache.cache) === 0
  ) {
    this._finalized = true;
    return encoding === "buffer" ? Buffer.from([]) : "";
  }
  if (TypedArrayPrototypeGetByteLength(this._cache.cache) != bs) {
    throw opensslError(
      "ERR_OSSL_EVP_WRONG_FINAL_BLOCK_LENGTH",
      "wrong final block length",
    );
  }

  if (this._autoPadding) {
    const padLen = TypedArrayPrototypeAt(buf, -1);
    if (padLen === 0 || padLen > bs) {
      throw opensslError(
        "ERR_OSSL_EVP_BAD_DECRYPT",
        "bad decrypt",
      );
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
  if (this._finalized) {
    throw new ERR_CRYPTO_INVALID_STATE("setAAD");
  }
  op_node_decipheriv_set_aad(this._context, buffer);
  return this;
};

Decipheriv.prototype.setAuthTag = function (
  buffer: any,
  _encoding?: string,
) {
  if (this._authTag) {
    throw new ERR_CRYPTO_INVALID_STATE("setAuthTag");
  }
  // When no explicit `authTagLength` was given at decipher creation time, a
  // GCM authentication tag must be the full 128 bits (16 bytes); shorter tags
  // are only accepted when `authTagLength` is set. This used to be the DEP0182
  // deprecation warning and is now a hard error (matching Node.js).
  // deno-lint-ignore prefer-primordials -- `buffer` may be Buffer/TypedArray/DataView
  const tagByteLength = buffer.byteLength;
  if (
    this._isGcmMode && this._authTagLength === -1 &&
    tagByteLength !== 16
  ) {
    throw new TypeError(
      `Invalid authentication tag length: ${tagByteLength}`,
    );
  }
  // deno-lint-ignore prefer-primordials -- `buffer` may be Buffer/TypedArray/DataView
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
  inputEncoding?: any,
  outputEncoding: any = getDefaultEncoding(),
): Buffer | string {
  if (this._finalized) {
    throw new ERR_CRYPTO_INVALID_STATE("update");
  }

  validateCipherUpdateData(data);

  let buf = data;
  if (typeof data === "string") {
    buf = Buffer.from(data, inputEncoding);
  }

  // Match Node.js/OpenSSL behavior: reject inputs >= INT_MAX bytes
  if (buf.length >= 2 ** 31 - 1) {
    throw new Error("Trying to add data in unsupported state");
  }

  _lazyInitDecipherDecoder(this, outputEncoding);

  if (this._isAesWrap) {
    const output = Buffer.from(
      op_node_aes_unwrap_key(
        this._aesWrapAlgorithm,
        this._aesWrapKey,
        this._aesWrapIv,
        buf,
      ),
    );
    if (outputEncoding !== "buffer") {
      return this._decoder!.write(output);
    }
    return output;
  }

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

const ENCRYPT_UNSUPPORTED_KEY_TYPES = new SafeSet([
  "rsa-pss",
  "dsa",
  "ec",
  "ed25519",
  "ed448",
  "x25519",
  "x448",
]);

function checkUnsupportedKeyType(key) {
  const keyType = isKeyObject(key)
    ? key.asymmetricKeyType
    : key?.key?.asymmetricKeyType;
  if (keyType && SetPrototypeHas(ENCRYPT_UNSUPPORTED_KEY_TYPES, keyType)) {
    throw new Error("operation not supported for this keytype");
  }
}

const WEBCRYPTO_SHA_HYPHEN_RE = new SafeRegExp("^(sha)-(?!3-)");

function normalizeOaepHash(hash: unknown): string | undefined {
  if (hash === undefined) return undefined;
  if (typeof hash !== "string") {
    throw new ERR_INVALID_ARG_TYPE("oaepHash", "string", hash);
  }
  if (!hash) return undefined;
  // Normalize to lowercase and strip WebCrypto-style hyphens
  // (e.g. "SHA-256" -> "sha256") but keep sha3/sha512 sub-variants
  // (e.g. "sha3-256", "sha512-224") intact.
  const normalized = StringPrototypeReplace(
    StringPrototypeToLowerCase(hash),
    WEBCRYPTO_SHA_HYPHEN_RE,
    "$1",
  );
  // Validate before key parsing so unknown hash throws ERR_OSSL_EVP_INVALID_DIGEST
  // even when the key itself cannot be parsed as a private key.
  op_node_validate_oaep_hash(normalized);
  return normalized;
}

function bufferEncodingFrom(keyOptions: unknown): string | undefined {
  return (keyOptions as { encoding?: string } | null)?.encoding;
}

function validateOaepLabel(
  label: unknown,
): ArrayBufferView | ArrayBuffer | undefined {
  if (label === undefined) return undefined;
  if (!isArrayBufferView(label) && !isAnyArrayBuffer(label)) {
    throw new ERR_INVALID_ARG_TYPE(
      "oaepLabel",
      ["Buffer", "TypedArray", "DataView"],
      label,
    );
  }
  return label as ArrayBufferView | ArrayBuffer;
}

function privateEncrypt(
  privateKey: ArrayBufferView | string | KeyObject,
  buffer: ArrayBufferView,
): Buffer {
  checkUnsupportedKeyType(privateKey);
  const { data } = prepareKey(privateKey);
  const padding = privateKey.padding || 1;
  const oaepHash = normalizeOaepHash(privateKey.oaepHash);
  const oaepLabel = validateOaepLabel(privateKey.oaepLabel);

  buffer = getArrayBufferOrView(
    buffer,
    "buffer",
    bufferEncodingFrom(privateKey),
  );
  return Buffer.from(
    op_node_private_encrypt(data, buffer, padding, oaepHash, oaepLabel),
  );
}

function privateDecrypt(
  privateKey: ArrayBufferView | string | KeyObject,
  buffer: ArrayBufferView,
): Buffer {
  checkUnsupportedKeyType(privateKey);
  const { data } = prepareKey(privateKey);
  // Node.js defaults privateDecrypt to RSA_PKCS1_OAEP_PADDING (4)
  const padding = privateKey.padding || 4;
  const oaepHash = normalizeOaepHash(privateKey.oaepHash);
  const oaepLabel = validateOaepLabel(privateKey.oaepLabel);

  buffer = getArrayBufferOrView(
    buffer,
    "buffer",
    bufferEncodingFrom(privateKey),
  );
  return Buffer.from(
    op_node_private_decrypt(data, buffer, padding, oaepHash, oaepLabel),
  );
}

function publicEncrypt(
  publicKey: ArrayBufferView | string | KeyObject,
  buffer: ArrayBufferView,
): Buffer {
  checkUnsupportedKeyType(publicKey);
  const { data } = prepareKey(publicKey);
  // Node.js defaults publicEncrypt to RSA_PKCS1_OAEP_PADDING (4)
  const padding = publicKey.padding || 4;
  const oaepHash = normalizeOaepHash(publicKey.oaepHash);
  const oaepLabel = validateOaepLabel(publicKey.oaepLabel);

  buffer = getArrayBufferOrView(
    buffer,
    "buffer",
    bufferEncodingFrom(publicKey),
  );
  return Buffer.from(
    op_node_public_encrypt(data, buffer, padding, oaepHash, oaepLabel),
  );
}

function prepareKey(key) {
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
    const { key: data, encoding, passphrase, format, type } = key;
    if (isKeyObject(data)) {
      return prepareKey(data);
    }
    if (format === "jwk") {
      // Build a KeyObject from the JWK and export it as PEM so the
      // downstream op can consume it via the existing PEM parsing path.
      const isPrivate = typeof data === "object" && data !== null &&
        typeof (data as { d?: unknown }).d === "string";
      const keyObject = isPrivate
        ? createPrivateKey({ key: data, format: "jwk" })
        : createPublicKey({ key: data, format: "jwk" });
      return prepareKey(keyObject);
    }
    if (!isStringOrBuffer(data)) {
      throw new TypeError("Invalid key type");
    }

    // If a passphrase is supplied with raw key material, decrypt the key via
    // the native key handle and re-export as unencrypted PKCS#8 PEM so the
    // downstream RSA ops can parse it.
    if (passphrase != null) {
      const keyFormat = format ?? (typeof data === "string" ? "pem" : "der");
      const keyData = getArrayBufferOrView(data, "key", encoding);
      const passphraseData = getArrayBufferOrView(passphrase, "passphrase");
      const handle = op_node_create_private_key(
        keyData,
        keyFormat,
        type ?? "",
        passphraseData,
      );
      const pem = op_node_export_private_key_pem(
        handle,
        "pkcs8",
        null,
        null,
      );
      return { data: getArrayBufferOrView(pem, "key") };
    }

    return { data: getArrayBufferOrView(data, "key", encoding) };
  }

  throw new TypeError("Invalid key type");
}

function publicDecrypt(
  publicKey: ArrayBufferView | string | KeyObject,
  buffer: ArrayBufferView,
): Buffer {
  checkUnsupportedKeyType(publicKey);
  const { data } = prepareKey(publicKey);
  const padding = publicKey.padding || 1;

  buffer = getArrayBufferOrView(
    buffer,
    "buffer",
    bufferEncodingFrom(publicKey),
  );
  return Buffer.from(op_node_public_decrypt(data, buffer, padding));
}

return {
  isStringOrBuffer,
  Cipheriv,
  Decipheriv,
  privateEncrypt,
  privateDecrypt,
  publicEncrypt,
  publicDecrypt,
  prepareKey,
  default: {
    privateDecrypt,
    privateEncrypt,
    publicDecrypt,
    publicEncrypt,
    Cipheriv,
    Decipheriv,
    prepareKey,
  },
};
})();

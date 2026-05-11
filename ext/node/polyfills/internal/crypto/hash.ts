// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials no-explicit-any

(function () {
const { core, primordials } = globalThis.__bootstrap;

const {
  Hasher,
  op_node_create_hash,
  op_node_export_secret_key,
  op_node_get_hashes,
  op_node_hash_clone,
  op_node_hash_digest,
  op_node_hash_digest_hex,
  op_node_hash_update,
  op_node_hash_update_str,
} = core.ops;

const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");

const lazyStream = core.createLazyLoader("node:stream");

const {
  forgivingBase64Encode: encodeToBase64,
  forgivingBase64UrlEncode: encodeToBase64Url,
} = core.loadExtScript("ext:deno_web/00_infra.js");
const {
  validateEncoding,
  validateString,
  validateUint32,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");
const {
  prepareSecretKey,
} = core.loadExtScript("ext:deno_node/internal/crypto/keys.ts");
const {
  ERR_CRYPTO_HASH_FINALIZED,
  ERR_INVALID_ARG_TYPE,
  NodeError,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");

const lazyLazyTransform = core.createLazyLoader(
  "ext:deno_node/internal/streams/lazy_transform.js",
);
const lazyProcess = core.createLazyLoader("node:process");

const {
  getDefaultEncoding,
  getHashBlockSize,
  toBuf,
} = core.loadExtScript("ext:deno_node/internal/crypto/util.ts");
const {
  isAnyArrayBuffer,
  isArrayBufferView,
} = core.loadExtScript("ext:deno_node/internal/util/types.ts");

const { ReflectApply, ObjectSetPrototypeOf } = primordials;

function unwrapErr(ok: boolean) {
  if (!ok) throw new ERR_CRYPTO_HASH_FINALIZED();
}

const kHandle = Symbol("kHandle");
const kFinalized = Symbol("kFinalized");

let warnedShakeOutputLength = false;

function Hash(
  algorithm: string | Hasher,
  options?: { outputLength?: number },
): Hash {
  if (!(this instanceof Hash)) {
    return new Hash(algorithm, options);
  }
  const isCopy = algorithm instanceof Hasher;
  if (!isCopy) {
    validateString(algorithm, "algorithm");
  }
  const xofLen = typeof options === "object" && options !== null
    ? options.outputLength
    : undefined;
  if (xofLen !== undefined) {
    validateUint32(xofLen, "options.outputLength");
  }

  const algoLower = isCopy ? undefined : algorithm.toLowerCase();

  if (
    !isCopy && xofLen === undefined &&
    (algoLower === "shake128" ||
      algoLower === "shake256") &&
    !warnedShakeOutputLength
  ) {
    warnedShakeOutputLength = true;
    const process = lazyProcess().default;
    process.emitWarning(
      "Creating SHAKE128/256 digests without an explicit options.outputLength is deprecated.",
      "DeprecationWarning",
      "DEP0198",
    );
  }

  try {
    this[kHandle] = isCopy
      ? op_node_hash_clone(algorithm, xofLen)
      : op_node_create_hash(algoLower, xofLen);
  } catch (err) {
    // TODO(lucacasonato): don't do this
    if (err.message === "Output length mismatch for non-extendable algorithm") {
      throw new NodeError(
        "ERR_OSSL_EVP_NOT_XOF_OR_INVALID_LENGTH",
        "Invalid XOF digest length",
      );
    } else {
      throw err;
    }
  }

  if (this[kHandle] === null) throw new ERR_CRYPTO_HASH_FINALIZED();

  const LazyTransform = lazyLazyTransform().default;
  ReflectApply(LazyTransform, this, [options]);
}

interface Hash {
  [kHandle]: object;
  [kFinalized]: boolean;
}

function _getLazyTransformProto() {
  const LazyTransform = lazyLazyTransform().default;
  return LazyTransform;
}

// Defer prototype chain setup
let _protoSetup = false;
function ensureProtoSetup() {
  if (_protoSetup) return;
  _protoSetup = true;
  const LazyTransform = _getLazyTransformProto();
  ObjectSetPrototypeOf(Hash.prototype, LazyTransform.prototype);
  ObjectSetPrototypeOf(Hash, LazyTransform);
}

Hash.prototype.copy = function copy(options?: { outputLength: number }) {
  return new Hash(this[kHandle], options);
};

Hash.prototype._transform = function _transform(
  chunk: string | Buffer,
  encoding: any,
  callback: () => void,
) {
  this.update(chunk, encoding);
  callback();
};

Hash.prototype._flush = function _flush(callback: () => void) {
  const digest = op_node_hash_digest(this[kHandle]);
  this.push(digest === null ? Buffer.alloc(0) : Buffer.from(digest));
  callback();
};

Hash.prototype.update = function update(
  data: string | Buffer,
  encoding: any,
) {
  encoding = encoding || getDefaultEncoding();

  if (typeof data === "string") {
    validateEncoding(data, encoding);
  } else if (!isArrayBufferView(data)) {
    throw new ERR_INVALID_ARG_TYPE(
      "data",
      ["string", "Buffer", "TypedArray", "DataView"],
      data,
    );
  }

  if (
    typeof data === "string" && (encoding === "utf8" || encoding === "buffer")
  ) {
    unwrapErr(op_node_hash_update_str(this[kHandle], data));
  } else {
    const buf = toBuf(data as string | Buffer, encoding);
    const u8 = buf instanceof Uint8Array ? buf : new Uint8Array(
      (buf as ArrayBufferView).buffer,
      (buf as ArrayBufferView).byteOffset,
      (buf as ArrayBufferView).byteLength,
    );
    unwrapErr(op_node_hash_update(this[kHandle], u8));
  }

  return this;
};

Hash.prototype.digest = function digest(outputEncoding: any) {
  if (this[kFinalized]) {
    throw new ERR_CRYPTO_HASH_FINALIZED();
  }
  outputEncoding = outputEncoding || getDefaultEncoding();
  outputEncoding = `${outputEncoding}`;

  if (outputEncoding === "hex") {
    const result = op_node_hash_digest_hex(this[kHandle]);
    if (result === null) throw new ERR_CRYPTO_HASH_FINALIZED();
    this[kFinalized] = true;
    return result;
  }

  const digest = op_node_hash_digest(this[kHandle]);
  if (digest === null) throw new ERR_CRYPTO_HASH_FINALIZED();
  this[kFinalized] = true;

  switch (outputEncoding) {
    case "binary":
      return String.fromCharCode(...digest);
    case "base64":
      return encodeToBase64(digest);
    case "base64url":
      return encodeToBase64Url(digest);
    case undefined:
    case "buffer":
      return Buffer.from(digest);
    default:
      return Buffer.from(digest).toString(outputEncoding);
  }
};

function Hmac(
  hmac: string,
  key: string | ArrayBuffer | KeyObject,
  options?: any,
): Hmac {
  return new HmacImpl(hmac, key, options);
}

type Hmac = HmacImpl;

let Transform;
function getTransform() {
  if (!Transform) Transform = lazyStream().Transform;
  return Transform;
}

class HmacImpl {
  #ipad: Uint8Array;
  #opad: Uint8Array;
  #ZEROES = Buffer.alloc(128);
  #algorithm: string;
  #hash: Hash;
  #finalized = false;

  constructor(
    hmac: string,
    key: string | ArrayBuffer,
    options?: any,
  ) {
    ensureHmacProtoSetup();
    const T = getTransform();
    // deno-lint-ignore no-this-alias
    const self = this;
    T.call(this, {
      transform(chunk: string, encoding: string, callback: () => void) {
        self.update(Buffer.from(chunk), encoding as any);
        callback();
      },
      flush(callback: () => void) {
        this.push(self.digest());
        callback();
      },
    });

    validateString(hmac, "hmac");

    key = prepareSecretKey(key, options?.encoding);
    let keyData;
    if (isArrayBufferView(key)) {
      keyData = key;
    } else if (isAnyArrayBuffer(key)) {
      keyData = new Uint8Array(key);
    } else {
      keyData = op_node_export_secret_key(key);
    }

    const alg = hmac.toLowerCase();
    this.#algorithm = alg;
    const blockSize = getHashBlockSize(alg);
    const keySize = keyData.length;

    let bufKey: Buffer;

    if (keySize > blockSize) {
      const hash = new Hash(alg, options);
      bufKey = hash.update(keyData).digest() as Buffer;
    } else {
      bufKey = Buffer.concat([keyData, this.#ZEROES], blockSize);
    }

    this.#ipad = Buffer.allocUnsafe(blockSize);
    this.#opad = Buffer.allocUnsafe(blockSize);

    for (let i = 0; i < blockSize; i++) {
      this.#ipad[i] = bufKey[i] ^ 0x36;
      this.#opad[i] = bufKey[i] ^ 0x5C;
    }

    this.#hash = new Hash(alg);
    this.#hash.update(this.#ipad);
  }

  digest(encoding?: any): Buffer | string {
    if (this.#finalized) {
      if (encoding && encoding !== "buffer") {
        return "";
      }
      return Buffer.alloc(0);
    }
    this.#finalized = true;

    const result = this.#hash.digest();

    return new Hash(this.#algorithm).update(this.#opad).update(result)
      .digest(
        encoding,
      );
  }

  update(data: string | ArrayBuffer, inputEncoding?: any): this {
    this.#hash.update(data, inputEncoding);
    return this;
  }
}

// Set up prototype chain after Transform is available
let _hmacProtoSetup = false;
function ensureHmacProtoSetup() {
  if (_hmacProtoSetup) return;
  _hmacProtoSetup = true;
  const T = getTransform();
  Object.setPrototypeOf(HmacImpl.prototype, T.prototype);
  Object.setPrototypeOf(HmacImpl, T);
}

Hmac.prototype = HmacImpl.prototype;

/**
 * Creates and returns a Hash object that can be used to generate hash digests
 * using the given `algorithm`. Optional `options` argument controls stream behavior.
 */
function createHash(algorithm: string, opts?: any) {
  ensureProtoSetup();
  return new Hash(algorithm, opts);
}

/**
 * Get the list of implemented hash algorithms.
 * @returns Array of hash algorithm names.
 */
function getHashes() {
  return op_node_get_hashes();
}

return {
  Hash,
  Hmac,
  createHash,
  getHashes,
  default: {
    Hash,
    Hmac,
    createHash,
  },
};
})();

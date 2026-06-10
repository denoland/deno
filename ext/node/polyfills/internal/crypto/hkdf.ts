// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// deno-lint-ignore-file no-explicit-any

(function () {
const { core, primordials } = __bootstrap;
const {
  PromisePrototypeCatch,
  PromisePrototypeThen,
  SafeSet,
  SetPrototypeHas,
  StringPrototypeToLowerCase,
  TypedArrayPrototypeGetBuffer,
  TypedArrayPrototypeGetByteLength,
  Uint8Array,
} = primordials;
const {
  op_node_get_hash_size,
  op_node_hkdf,
  op_node_hkdf_async,
} = core.ops;

const {
  validateFunction,
  validateInteger,
  validateString,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");
const {
  ERR_CRYPTO_INVALID_DIGEST,
  ERR_CRYPTO_INVALID_KEYLEN,
  ERR_INVALID_ARG_TYPE,
  ERR_OUT_OF_RANGE,
  hideStackFrames,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const {
  kHandle,
  toBuf,
  validateByteSource,
} = core.loadExtScript("ext:deno_node/internal/crypto/util.ts");
const {
  createSecretKey,
} = core.loadExtScript("ext:deno_node/internal/crypto/keys.ts");
const { kMaxLength } = core.loadExtScript(
  "ext:deno_node/internal/buffer.mjs",
);
const {
  isAnyArrayBuffer,
  isArrayBufferView,
} = core.loadExtScript("ext:deno_node/internal/util/types.ts");
const { isKeyObject } = core.loadExtScript(
  "ext:deno_node/internal/crypto/_keys.ts",
);
const { getHashes } = core.loadExtScript(
  "ext:deno_node/internal/crypto/hash.ts",
);
const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");

// Consume raw bytes for any ArrayBufferView/ArrayBuffer; strings via toBuf.
function toRawBytes(x: unknown): Buffer {
  if (isArrayBufferView(x)) {
    const v = x as ArrayBufferView;
    // deno-lint-ignore prefer-primordials -- v is ArrayBufferView (TypedArray | DataView); buffer/byteOffset/byteLength getters are polymorphic
    return Buffer.from(v.buffer, v.byteOffset, v.byteLength);
  }
  if (isAnyArrayBuffer(x)) {
    return Buffer.from(x as ArrayBufferLike);
  }
  // For strings / other BinaryLike, keep existing semantics (UTF-8 etc.)
  return Buffer.from(toBuf(x as unknown as string));
}

const validateParameters = hideStackFrames(
  (hash, key, salt, info, length) => {
    validateString(hash, "digest");
    key = prepareKey(key);
    validateByteSource(salt, "salt");
    validateByteSource(info, "info");

    salt = toRawBytes(toBuf(salt));
    info = toRawBytes(toBuf(info));

    validateInteger(length, "length", 0, kMaxLength);

    if (TypedArrayPrototypeGetByteLength(info) > 1024) {
      throw new ERR_OUT_OF_RANGE(
        "info",
        "must not contain more than 1024 bytes",
        TypedArrayPrototypeGetByteLength(info),
      );
    }

    validateAlgorithm(hash);

    const size = op_node_get_hash_size(hash);
    if (typeof size === "number" && size * 255 < length) {
      throw new ERR_CRYPTO_INVALID_KEYLEN();
    }

    return {
      hash,
      key,
      salt,
      info,
      length,
    };
  },
);

function prepareKey(key: any) {
  if (isKeyObject(key)) {
    return key;
  }

  if (isAnyArrayBuffer(key)) {
    return createSecretKey(new Uint8Array(key as unknown as ArrayBufferLike));
  }

  key = toBuf(key as string);

  if (!isArrayBufferView(key)) {
    throw new ERR_INVALID_ARG_TYPE(
      "ikm",
      [
        "string",
        "SecretKeyObject",
        "ArrayBuffer",
        "TypedArray",
        "DataView",
        "Buffer",
      ],
      key,
    );
  }

  return createSecretKey(key);
}

function hkdf(
  hash: string,
  key: any,
  salt: any,
  info: any,
  length: number,
  callback: (err: Error | null, derivedKey: ArrayBuffer | undefined) => void,
) {
  ({ hash, key, salt, info, length } = validateParameters(
    hash,
    key,
    salt,
    info,
    length,
  ));

  validateFunction(callback, "callback");

  hash = StringPrototypeToLowerCase(hash);

  PromisePrototypeCatch(
    PromisePrototypeThen(
      op_node_hkdf_async(hash, key[kHandle], salt, info, length),
      (okm) => callback(null, TypedArrayPrototypeGetBuffer(okm)),
    ),
    (err) => callback(new ERR_CRYPTO_INVALID_DIGEST(err), undefined),
  );
}

function hkdfSync(
  hash: string,
  key: any,
  salt: any,
  info: any,
  length: number,
) {
  ({ hash, key, salt, info, length } = validateParameters(
    hash,
    key,
    salt,
    info,
    length,
  ));

  hash = StringPrototypeToLowerCase(hash);

  const okm = new Uint8Array(length);
  try {
    op_node_hkdf(hash, key[kHandle], salt, info, okm);
  } catch (e) {
    throw new ERR_CRYPTO_INVALID_DIGEST(e);
  }

  return TypedArrayPrototypeGetBuffer(okm);
}

let hashes: Set<string> | null = null;
function validateAlgorithm(algorithm: string) {
  if (hashes === null) {
    hashes = new SafeSet(getHashes());
  }

  if (!SetPrototypeHas(hashes, algorithm)) {
    throw new ERR_CRYPTO_INVALID_DIGEST(algorithm);
  }
}

return {
  hkdf,
  hkdfSync,
  default: {
    hkdf,
    hkdfSync,
  },
};
})();

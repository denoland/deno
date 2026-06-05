// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials, internals } = __bootstrap;
const {
  isArrayBuffer,
  isTypedArray,
  isDataView,
} = core;
const {
  op_crypto_base64url_decode,
  op_crypto_base64url_encode,
  op_crypto_decrypt,
  op_crypto_derive_bits,
  op_crypto_derive_bits_x25519,
  op_crypto_derive_bits_x448,
  op_crypto_encrypt,
  op_crypto_export_key,
  op_crypto_export_pkcs8_ed25519,
  op_crypto_export_pkcs8_x25519,
  op_crypto_export_pkcs8_x448,
  op_crypto_export_spki_ed25519,
  op_crypto_export_spki_x25519,
  op_crypto_export_spki_x448,
  op_crypto_generate_ed25519_keypair,
  op_crypto_generate_key,
  op_crypto_generate_x25519_keypair,
  op_crypto_generate_x448_keypair,
  op_crypto_get_random_values,
  op_crypto_import_key,
  op_crypto_key_store_get,
  op_crypto_key_store_insert,
  op_crypto_import_pkcs8_ed25519,
  op_crypto_import_pkcs8_x25519,
  op_crypto_import_pkcs8_x448,
  op_crypto_import_spki_ed25519,
  op_crypto_import_spki_x25519,
  op_crypto_import_spki_x448,
  op_crypto_jwk_x_ed25519,
  op_crypto_ml_kem_decapsulate,
  op_crypto_ml_kem_encapsulate,
  op_crypto_ml_kem_export_pkcs8,
  op_crypto_ml_kem_export_spki,
  op_crypto_ml_kem_from_seed,
  op_crypto_ml_kem_get_public_key,
  op_crypto_ml_kem_import_pkcs8,
  op_crypto_ml_kem_import_spki,
  op_crypto_ml_kem_validate_private_key,
  op_crypto_ml_kem_validate_public_key,
  op_crypto_mldsa_export_pkcs8,
  op_crypto_mldsa_export_spki,
  op_crypto_mldsa_from_pkcs8,
  op_crypto_mldsa_from_raw_private,
  op_crypto_mldsa_from_seed,
  op_crypto_mldsa_from_spki,
  op_crypto_random_uuid,
  op_crypto_sign_ed25519,
  op_crypto_sign_key,
  op_crypto_sign_mldsa,
  op_crypto_subtle_digest,
  op_crypto_subtle_digest_xof,
  op_crypto_unwrap_key,
  op_crypto_verify_ed25519,
  op_crypto_verify_key,
  op_crypto_verify_mldsa,
  op_crypto_wrap_key,
  op_crypto_x25519_public_key,
  op_crypto_x448_public_key,
} = core.ops;
const {
  ArrayBufferIsView,
  ArrayBufferPrototypeGetByteLength,
  ArrayBufferPrototypeSlice,
  ArrayPrototypeEvery,
  ArrayPrototypeFilter,
  ArrayPrototypeFind,
  ArrayPrototypeIncludes,
  DataViewPrototypeGetBuffer,
  DataViewPrototypeGetByteLength,
  DataViewPrototypeGetByteOffset,
  JSONParse,
  JSONStringify,
  MathCeil,
  ObjectAssign,
  ObjectDefineProperty,
  ObjectHasOwn,
  ObjectPrototypeIsPrototypeOf,
  SafeArrayIterator,
  SafeWeakMap,
  StringFromCharCode,
  StringPrototypeCharCodeAt,
  StringPrototypeToLowerCase,
  StringPrototypeToUpperCase,
  Symbol,
  SymbolFor,
  SyntaxError,
  TypeError,
  TypedArrayPrototypeGetBuffer,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeGetByteOffset,
  TypedArrayPrototypeGetSymbolToStringTag,
  TypedArrayPrototypeSlice,
  Uint8Array,
  WeakMapPrototypeGet,
  WeakMapPrototypeSet,
} = primordials;

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const { createFilteredInspectProxy } = core.loadExtScript(
  "ext:deno_web/01_console.js",
);
const { DOMException } = core.loadExtScript("ext:deno_web/01_dom_exception.js");
const { kKeyObject } = internals;

const supportedNamedCurves = ["P-256", "P-384", "P-521"];
const recognisedUsages = [
  "encrypt",
  "decrypt",
  "sign",
  "verify",
  "deriveKey",
  "deriveBits",
  "wrapKey",
  "unwrapKey",
  "encapsulateKey",
  "encapsulateBits",
  "decapsulateKey",
  "decapsulateBits",
];

const simpleAlgorithmDictionaries = {
  AesGcmParams: { iv: "BufferSource", additionalData: "BufferSource" },
  RsaHashedKeyGenParams: { hash: "HashAlgorithmIdentifier" },
  EcKeyGenParams: {},
  HmacKeyGenParams: { hash: "HashAlgorithmIdentifier" },
  RsaPssParams: {},
  EcdsaParams: { hash: "HashAlgorithmIdentifier" },
  HmacImportParams: { hash: "HashAlgorithmIdentifier" },
  HkdfParams: {
    hash: "HashAlgorithmIdentifier",
    salt: "BufferSource",
    info: "BufferSource",
  },
  Pbkdf2Params: { hash: "HashAlgorithmIdentifier", salt: "BufferSource" },
  RsaOaepParams: { label: "BufferSource" },
  RsaHashedImportParams: { hash: "HashAlgorithmIdentifier" },
  EcKeyImportParams: {},
  ChaCha20Poly1305Params: {
    iv: "BufferSource",
    additionalData: "BufferSource",
  },
  ShakeParams: {},
  CShakeParams: {
    functionName: "BufferSource",
    customization: "BufferSource",
  },
  TurboShakeParams: {},
  MlDsaParams: { context: "BufferSource" },
};

const supportedAlgorithms = {
  "digest": {
    "SHA-1": null,
    "SHA-256": null,
    "SHA-384": null,
    "SHA-512": null,
    "SHA3-256": null,
    "SHA3-384": null,
    "SHA3-512": null,
    "SHAKE128": "ShakeParams",
    "SHAKE256": "ShakeParams",
    "cSHAKE128": "CShakeParams",
    "cSHAKE256": "CShakeParams",
    "TurboSHAKE128": "TurboShakeParams",
    "TurboSHAKE256": "TurboShakeParams",
  },
  "generateKey": {
    "RSASSA-PKCS1-v1_5": "RsaHashedKeyGenParams",
    "RSA-PSS": "RsaHashedKeyGenParams",
    "RSA-OAEP": "RsaHashedKeyGenParams",
    "ECDSA": "EcKeyGenParams",
    "ECDH": "EcKeyGenParams",
    "AES-CTR": "AesKeyGenParams",
    "AES-CBC": "AesKeyGenParams",
    "AES-GCM": "AesKeyGenParams",
    "AES-OCB": "AesKeyGenParams",
    "AES-KW": "AesKeyGenParams",
    "HMAC": "HmacKeyGenParams",
    "ChaCha20-Poly1305": null,
    "X25519": null,
    "X448": null,
    "Ed25519": null,
    "ML-KEM-512": null,
    "ML-KEM-768": null,
    "ML-KEM-1024": null,
    "ML-DSA-44": null,
    "ML-DSA-65": null,
    "ML-DSA-87": null,
  },
  "sign": {
    "RSASSA-PKCS1-v1_5": null,
    "RSA-PSS": "RsaPssParams",
    "ECDSA": "EcdsaParams",
    "HMAC": null,
    "Ed25519": null,
    "ML-DSA-44": "MlDsaParams",
    "ML-DSA-65": "MlDsaParams",
    "ML-DSA-87": "MlDsaParams",
  },
  "verify": {
    "RSASSA-PKCS1-v1_5": null,
    "RSA-PSS": "RsaPssParams",
    "ECDSA": "EcdsaParams",
    "HMAC": null,
    "Ed25519": null,
    "ML-DSA-44": "MlDsaParams",
    "ML-DSA-65": "MlDsaParams",
    "ML-DSA-87": "MlDsaParams",
  },
  "importKey": {
    "RSASSA-PKCS1-v1_5": "RsaHashedImportParams",
    "RSA-PSS": "RsaHashedImportParams",
    "RSA-OAEP": "RsaHashedImportParams",
    "ECDSA": "EcKeyImportParams",
    "ECDH": "EcKeyImportParams",
    "HMAC": "HmacImportParams",
    "HKDF": null,
    "PBKDF2": null,
    "AES-CTR": null,
    "AES-CBC": null,
    "AES-GCM": null,
    "AES-OCB": null,
    "AES-KW": null,
    "ChaCha20-Poly1305": null,
    "Ed25519": null,
    "X25519": null,
    "X448": null,
    "ML-KEM-512": null,
    "ML-KEM-768": null,
    "ML-KEM-1024": null,
    "ML-DSA-44": null,
    "ML-DSA-65": null,
    "ML-DSA-87": null,
  },
  "encapsulate": {
    "ML-KEM-512": null,
    "ML-KEM-768": null,
    "ML-KEM-1024": null,
  },
  "decapsulate": {
    "ML-KEM-512": null,
    "ML-KEM-768": null,
    "ML-KEM-1024": null,
  },
  "deriveBits": {
    "HKDF": "HkdfParams",
    "PBKDF2": "Pbkdf2Params",
    "ECDH": "EcdhKeyDeriveParams",
    "X25519": "EcdhKeyDeriveParams",
    "X448": "EcdhKeyDeriveParams",
  },
  "encrypt": {
    "RSA-OAEP": "RsaOaepParams",
    "AES-CBC": "AesCbcParams",
    "AES-GCM": "AesGcmParams",
    "AES-OCB": "AesGcmParams",
    "AES-CTR": "AesCtrParams",
    "ChaCha20-Poly1305": "ChaCha20Poly1305Params",
  },
  "decrypt": {
    "RSA-OAEP": "RsaOaepParams",
    "AES-CBC": "AesCbcParams",
    "AES-GCM": "AesGcmParams",
    "AES-OCB": "AesGcmParams",
    "AES-CTR": "AesCtrParams",
    "ChaCha20-Poly1305": "ChaCha20Poly1305Params",
  },
  "get key length": {
    "AES-CBC": "AesDerivedKeyParams",
    "AES-CTR": "AesDerivedKeyParams",
    "AES-GCM": "AesDerivedKeyParams",
    "AES-KW": "AesDerivedKeyParams",
    "HMAC": "HmacImportParams",
    "ChaCha20-Poly1305": null,
    "HKDF": null,
    "PBKDF2": null,
  },
  "wrapKey": {
    "AES-KW": null,
  },
  "unwrapKey": {
    "AES-KW": null,
  },
};

const aesJwkAlg = {
  "AES-CTR": {
    128: "A128CTR",
    192: "A192CTR",
    256: "A256CTR",
  },
  "AES-CBC": {
    128: "A128CBC",
    192: "A192CBC",
    256: "A256CBC",
  },
  "AES-GCM": {
    128: "A128GCM",
    192: "A192GCM",
    256: "A256GCM",
  },
  "AES-KW": {
    128: "A128KW",
    192: "A192KW",
    256: "A256KW",
  },
};

// See https://www.w3.org/TR/WebCryptoAPI/#dfn-normalize-an-algorithm
// 18.4.4
function normalizeAlgorithm(algorithm, op) {
  if (typeof algorithm == "string") {
    return normalizeAlgorithm({ name: algorithm }, op);
  }

  // 1.
  const registeredAlgorithms = supportedAlgorithms[op];
  // 2. 3.
  const initialAlg = webidl.converters.Algorithm(
    algorithm,
    "Failed to normalize algorithm",
    "passed algorithm",
  );
  // 4.
  let algName = initialAlg.name;

  // 5.
  let desiredType = undefined;
  for (const key in registeredAlgorithms) {
    if (!ObjectHasOwn(registeredAlgorithms, key)) {
      continue;
    }
    if (
      StringPrototypeToUpperCase(key) === StringPrototypeToUpperCase(algName)
    ) {
      algName = key;
      desiredType = registeredAlgorithms[key];
    }
  }
  if (desiredType === undefined) {
    throw new DOMException(
      "Unrecognized algorithm name",
      "NotSupportedError",
    );
  }

  // Fast path everything below if the registered dictionary is "None".
  if (desiredType === null) {
    return { name: algName };
  }

  // 6.
  const normalizedAlgorithm = webidl.converters[desiredType](
    algorithm,
    "Failed to normalize algorithm",
    "passed algorithm",
  );
  // 7.
  normalizedAlgorithm.name = algName;

  // 9.
  const dict = simpleAlgorithmDictionaries[desiredType];
  // 10.
  for (const member in dict) {
    if (!ObjectHasOwn(dict, member)) {
      continue;
    }
    const idlType = dict[member];
    const idlValue = normalizedAlgorithm[member];
    // 3.
    if (idlType === "BufferSource" && idlValue) {
      normalizedAlgorithm[member] = copyBuffer(idlValue);
    } else if (idlType === "HashAlgorithmIdentifier") {
      normalizedAlgorithm[member] = normalizeAlgorithm(idlValue, "digest");
    } else if (idlType === "AlgorithmIdentifier") {
      // TODO(lucacasonato): implement
      throw new TypeError("Unimplemented");
    }
  }

  return normalizedAlgorithm;
}

/**
 * @param {ArrayBufferView | ArrayBuffer} input
 * @returns {Uint8Array}
 */
function copyBuffer(input) {
  if (isTypedArray(input)) {
    return TypedArrayPrototypeSlice(
      new Uint8Array(
        TypedArrayPrototypeGetBuffer(/** @type {Uint8Array} */ (input)),
        TypedArrayPrototypeGetByteOffset(/** @type {Uint8Array} */ (input)),
        TypedArrayPrototypeGetByteLength(/** @type {Uint8Array} */ (input)),
      ),
    );
  } else if (isDataView(input)) {
    return TypedArrayPrototypeSlice(
      new Uint8Array(
        DataViewPrototypeGetBuffer(/** @type {DataView} */ (input)),
        DataViewPrototypeGetByteOffset(/** @type {DataView} */ (input)),
        DataViewPrototypeGetByteLength(/** @type {DataView} */ (input)),
      ),
    );
  }
  // ArrayBuffer
  return TypedArrayPrototypeSlice(
    new Uint8Array(
      input,
      0,
      ArrayBufferPrototypeGetByteLength(input),
    ),
  );
}

const _handle = Symbol("[[handle]]");
const _algorithm = Symbol("[[algorithm]]");
const _extractable = Symbol("[[extractable]]");
const _usages = Symbol("[[usages]]");
const _type = Symbol("[[type]]");

class CryptoKey {
  /** @type {string} */
  [_type];
  /** @type {boolean} */
  [_extractable];
  /** @type {object} */
  [_algorithm];
  /** @type {string[]} */
  [_usages];
  /** @type {object} */
  [_handle];
  /** @type {object} */
  [kKeyObject];

  constructor() {
    webidl.illegalConstructor();
  }

  /** @returns {string} */
  get type() {
    webidl.assertBranded(this, CryptoKeyPrototype);
    return this[_type];
  }

  /** @returns {boolean} */
  get extractable() {
    webidl.assertBranded(this, CryptoKeyPrototype);
    return this[_extractable];
  }

  /** @returns {string[]} */
  get usages() {
    webidl.assertBranded(this, CryptoKeyPrototype);
    // TODO(lucacasonato): return a SameObject copy
    return this[_usages];
  }

  /** @returns {object} */
  get algorithm() {
    webidl.assertBranded(this, CryptoKeyPrototype);
    // TODO(lucacasonato): return a SameObject copy
    return this[_algorithm];
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(CryptoKeyPrototype, this),
        keys: [
          "type",
          "extractable",
          "algorithm",
          "usages",
        ],
      }),
      inspectOptions,
    );
  }
}

webidl.configureInterface(CryptoKey);
const CryptoKeyPrototype = CryptoKey.prototype;

/**
 * @param {string} type
 * @param {boolean} extractable
 * @param {string[]} usages
 * @param {object} algorithm
 * @param {object} handle
 * @returns
 */
function constructKey(type, extractable, usages, algorithm, handle) {
  const key = webidl.createBranded(CryptoKey);
  key[_type] = type;
  key[_extractable] = extractable;
  key[_usages] = usages;
  key[_algorithm] = algorithm;
  key[_handle] = handle;
  key[kKeyObject] = getKeyData(handle);
  ObjectDefineProperty(key, core.hostObjectBrand, {
    __proto__: null,
    value: () => ({
      type: "CryptoKey",
      keyType: type,
      extractable,
      usages,
      algorithm,
      keyData: getKeyData(handle),
    }),
    enumerable: false,
    configurable: false,
    writable: false,
  });
  return key;
}

core.registerCloneableResource("CryptoKey", (data) => {
  const handle = {};
  setKeyData(handle, data.keyData);
  return constructKey(
    data.keyType,
    data.extractable,
    data.usages,
    data.algorithm,
    handle,
  );
});

// https://w3c.github.io/webcrypto/#concept-usage-intersection
/**
 * @param {string[]} a
 * @param {string[]} b
 * @returns
 */
function usageIntersection(a, b) {
  return ArrayPrototypeFilter(
    a,
    (i) => ArrayPrototypeIncludes(b, i),
  );
}

/**
 * Throw a SyntaxError if any requested usage is not valid for a public key of
 * the algorithm (i.e. is not present in `allowed`).
 *
 * @param {string[]} requested
 * @param {string[]} allowed
 */
function validatePublicKeyUsages(requested, allowed) {
  for (let i = 0; i < requested.length; i++) {
    if (!ArrayPrototypeIncludes(allowed, requested[i])) {
      throw new DOMException("Invalid key usage", "SyntaxError");
    }
  }
}

// The key material for every CryptoKey lives in Rust, wrapped in a V8
// garbage-collected (cppgc) object created by `op_crypto_key_store_insert`. A
// `handle` here is a plain object that holds that cppgc object on its `cppgc`
// property and is shared between the CryptoKey(s) that reference the same key
// material (e.g. the public and private key of a key pair).
//
// Because the cppgc object is referenced only through the handle, V8's garbage
// collector frees the underlying Rust key material automatically once the handle
// (and therefore every CryptoKey referencing it) is collected. No
// `FinalizationRegistry` or manual bookkeeping is required.

/**
 * Store key material in a Rust-side cppgc object referenced by `handle.cppgc`.
 *
 * `value` is one of:
 *  - a `{ type, data }` object (secret/private/public key material),
 *  - a raw `Uint8Array`/`ArrayBuffer` of key bytes (Ed25519/X25519/X448 and
 *    ML-KEM/ML-DSA public keys),
 *  - a composite `{ seed, privateKey }` object holding the expanded private key
 *    bytes and the seed it was derived from (ML-KEM/ML-DSA private keys).
 *
 * @param {{ cppgc?: object }} handle
 * @param {{ type: string, data: Uint8Array } | Uint8Array | object} value
 */
function setKeyData(handle, value) {
  let payload;
  if (value.type !== undefined) {
    payload = { kind: value.type, data: value.data };
  } else if (ArrayBufferIsView(value) || isArrayBuffer(value)) {
    payload = { kind: "raw", data: value };
  } else {
    payload = {
      kind: "seeded",
      seed: value.seed,
      privateKey: value.privateKey,
    };
  }
  handle.cppgc = op_crypto_key_store_insert(payload);
}

/**
 * Read key material back, returning the same shape that was passed to
 * {@linkcode setKeyData} (a `{ type, data }` object, a raw `Uint8Array`, or a
 * composite `{ seed, privateKey }` object). Used for key export, structured
 * clone and node:crypto interop, and by the ops that still take key bytes
 * directly.
 *
 * @param {{ cppgc: object }} handle
 * @returns {{ type: string, data: Uint8Array } | Uint8Array | object}
 */
function getKeyData(handle) {
  const value = op_crypto_key_store_get(handle.cppgc);
  switch (value.kind) {
    case "raw":
      return value.data;
    case "seeded":
      return { seed: value.seed, privateKey: value.privateKey };
    default:
      return { type: value.kind, data: value.data };
  }
}

/** @type {WeakMap<CryptoKey, CryptoKey>} */
const MLDSA_PUBLIC_FROM_PRIVATE = new SafeWeakMap();

function mldsaVariantId(name) {
  switch (name) {
    case "ML-DSA-44":
      return 0;
    case "ML-DSA-65":
      return 1;
    case "ML-DSA-87":
      return 2;
    default:
      throw new TypeError(`Unknown ML-DSA variant: ${name}`);
  }
}

function mldsaPublicKeyLen(variant) {
  switch (variant) {
    case 0:
      return 1312;
    case 1:
      return 1952;
    case 2:
      return 2592;
    default:
      throw new TypeError("Unknown ML-DSA variant");
  }
}

function bytesEqual(a, b) {
  const len = TypedArrayPrototypeGetByteLength(a);
  if (len !== TypedArrayPrototypeGetByteLength(b)) {
    return false;
  }
  for (let i = 0; i < len; i++) {
    if (a[i] !== b[i]) {
      return false;
    }
  }
  return true;
}

function getKeyLength(algorithm) {
  switch (algorithm.name) {
    case "AES-CBC":
    case "AES-CTR":
    case "AES-GCM":
    case "AES-OCB":
    case "AES-KW": {
      // 1.
      if (!ArrayPrototypeIncludes([128, 192, 256], algorithm.length)) {
        throw new DOMException(
          `Length must be 128, 192, or 256: received ${algorithm.length}`,
          "OperationError",
        );
      }

      // 2.
      return algorithm.length;
    }
    case "HMAC": {
      // 1.
      let length;
      if (algorithm.length === undefined) {
        switch (algorithm.hash.name) {
          case "SHA-1":
            length = 512;
            break;
          case "SHA-256":
            length = 512;
            break;
          case "SHA-384":
            length = 1024;
            break;
          case "SHA-512":
            length = 1024;
            break;
          case "SHA3-256":
            length = 512;
            break;
          case "SHA3-384":
            length = 1024;
            break;
          case "SHA3-512":
            length = 1024;
            break;
          default:
            throw new DOMException(
              `Unrecognized hash algorithm: ${algorithm.hash.name}`,
              "NotSupportedError",
            );
        }
      } else if (algorithm.length !== 0) {
        length = algorithm.length;
      } else {
        throw new TypeError(`Invalid length: ${algorithm.length}`);
      }

      // 2.
      return length;
    }
    case "ChaCha20-Poly1305": {
      // ChaCha20-Poly1305 keys are always 256 bits.
      return 256;
    }
    case "HKDF": {
      // 1.
      return null;
    }
    case "PBKDF2": {
      // 1.
      return null;
    }
    default:
      throw new TypeError("Unreachable");
  }
}

class SubtleCrypto {
  constructor() {
    webidl.illegalConstructor();
  }

  /**
   * @param {string} algorithm
   * @param {BufferSource} data
   * @returns {Promise<ArrayBuffer>}
   */
  async digest(algorithm, data) {
    webidl.assertBranded(this, SubtleCryptoPrototype);
    const prefix = "Failed to execute 'digest' on 'SubtleCrypto'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    algorithm = webidl.converters.AlgorithmIdentifier(
      algorithm,
      prefix,
      "Argument 1",
    );
    data = webidl.converters.BufferSource(data, prefix, "Argument 2");

    data = copyBuffer(data);

    algorithm = normalizeAlgorithm(algorithm, "digest");

    switch (algorithm.name) {
      case "SHAKE128":
      case "SHAKE256":
      case "cSHAKE128":
      case "cSHAKE256":
      case "TurboSHAKE128":
      case "TurboSHAKE256": {
        if (
          algorithm.outputLength === undefined || algorithm.outputLength === 0
        ) {
          throw new DOMException(
            `'outputLength' must be a positive multiple of 8 for ${algorithm.name}`,
            "OperationError",
          );
        }
        if (algorithm.outputLength % 8 !== 0) {
          throw new DOMException(
            `'outputLength' must be a multiple of 8 for ${algorithm.name}`,
            "OperationError",
          );
        }
        if (
          (algorithm.name === "TurboSHAKE128" ||
            algorithm.name === "TurboSHAKE256") &&
          algorithm.domainSeparation !== undefined &&
          (algorithm.domainSeparation < 0x01 ||
            algorithm.domainSeparation > 0x7F)
        ) {
          throw new DOMException(
            "'domainSeparation' must be in [0x01, 0x7F]",
            "OperationError",
          );
        }
        const xofResult = await op_crypto_subtle_digest_xof({
          name: algorithm.name,
          outputLength: algorithm.outputLength,
          functionName: algorithm.functionName ?? null,
          customization: algorithm.customization ?? null,
          domainSeparation: algorithm.domainSeparation ?? null,
        }, data);
        return TypedArrayPrototypeGetBuffer(xofResult);
      }
    }

    const result = await op_crypto_subtle_digest(
      algorithm.name,
      data,
    );

    return TypedArrayPrototypeGetBuffer(result);
  }

  /**
   * @param {string} algorithm
   * @param {CryptoKey} key
   * @param {BufferSource} data
   * @returns {Promise<any>}
   */
  async encrypt(algorithm, key, data) {
    webidl.assertBranded(this, SubtleCryptoPrototype);
    const prefix = "Failed to execute 'encrypt' on 'SubtleCrypto'";
    webidl.requiredArguments(arguments.length, 3, prefix);
    algorithm = webidl.converters.AlgorithmIdentifier(
      algorithm,
      prefix,
      "Argument 1",
    );
    key = webidl.converters.CryptoKey(key, prefix, "Argument 2");
    data = webidl.converters.BufferSource(data, prefix, "Argument 3");

    // 2.
    data = copyBuffer(data);

    // 3.
    const normalizedAlgorithm = normalizeAlgorithm(algorithm, "encrypt");

    // 8.
    if (normalizedAlgorithm.name !== key[_algorithm].name) {
      throw new DOMException(
        `Encryption algorithm '${normalizedAlgorithm.name}' does not match key algorithm`,
        "InvalidAccessError",
      );
    }

    // 9.
    if (!ArrayPrototypeIncludes(key[_usages], "encrypt")) {
      throw new DOMException(
        "The requested operation is not valid for the provided key",
        "InvalidAccessError",
      );
    }

    return await encrypt(normalizedAlgorithm, key, data);
  }

  /**
   * @param {string} algorithm
   * @param {CryptoKey} key
   * @param {BufferSource} data
   * @returns {Promise<any>}
   */
  async decrypt(algorithm, key, data) {
    webidl.assertBranded(this, SubtleCryptoPrototype);
    const prefix = "Failed to execute 'decrypt' on 'SubtleCrypto'";
    webidl.requiredArguments(arguments.length, 3, prefix);
    algorithm = webidl.converters.AlgorithmIdentifier(
      algorithm,
      prefix,
      "Argument 1",
    );
    key = webidl.converters.CryptoKey(key, prefix, "Argument 2");
    data = webidl.converters.BufferSource(data, prefix, "Argument 3");

    // 2.
    data = copyBuffer(data);

    // 3.
    const normalizedAlgorithm = normalizeAlgorithm(algorithm, "decrypt");

    // 8.
    if (normalizedAlgorithm.name !== key[_algorithm].name) {
      throw new DOMException(
        `Decryption algorithm "${normalizedAlgorithm.name}" does not match key algorithm`,
        "OperationError",
      );
    }

    // 9.
    if (!ArrayPrototypeIncludes(key[_usages], "decrypt")) {
      throw new DOMException(
        "The requested operation is not valid for the provided key",
        "InvalidAccessError",
      );
    }

    const handle = key[_handle];

    switch (normalizedAlgorithm.name) {
      case "RSA-OAEP": {
        // 1.
        if (key[_type] !== "private") {
          throw new DOMException(
            "Key type not supported",
            "InvalidAccessError",
          );
        }

        // 2.
        if (normalizedAlgorithm.label) {
          normalizedAlgorithm.label = copyBuffer(normalizedAlgorithm.label);
        } else {
          normalizedAlgorithm.label = new Uint8Array();
        }

        // 3-5.
        const hashAlgorithm = key[_algorithm].hash.name;
        const plainText = await op_crypto_decrypt(handle.cppgc, {
          algorithm: "RSA-OAEP",
          hash: hashAlgorithm,
          label: normalizedAlgorithm.label,
        }, data);

        // 6.
        return TypedArrayPrototypeGetBuffer(plainText);
      }
      case "AES-CBC": {
        normalizedAlgorithm.iv = copyBuffer(normalizedAlgorithm.iv);

        // 1.
        if (TypedArrayPrototypeGetByteLength(normalizedAlgorithm.iv) !== 16) {
          throw new DOMException(
            "Counter must be 16 bytes",
            "OperationError",
          );
        }

        const plainText = await op_crypto_decrypt(handle.cppgc, {
          algorithm: "AES-CBC",
          iv: normalizedAlgorithm.iv,
          length: key[_algorithm].length,
        }, data);

        // 6.
        return TypedArrayPrototypeGetBuffer(plainText);
      }
      case "AES-CTR": {
        normalizedAlgorithm.counter = copyBuffer(normalizedAlgorithm.counter);

        // 1.
        if (
          TypedArrayPrototypeGetByteLength(normalizedAlgorithm.counter) !== 16
        ) {
          throw new DOMException(
            "Counter vector must be 16 bytes",
            "OperationError",
          );
        }

        // 2.
        if (
          normalizedAlgorithm.length === 0 || normalizedAlgorithm.length > 128
        ) {
          throw new DOMException(
            `Counter length must not be 0 or greater than 128: received ${normalizedAlgorithm.length}`,
            "OperationError",
          );
        }

        // 3.
        const cipherText = await op_crypto_decrypt(handle.cppgc, {
          algorithm: "AES-CTR",
          keyLength: key[_algorithm].length,
          counter: normalizedAlgorithm.counter,
          ctrLength: normalizedAlgorithm.length,
        }, data);

        // 4.
        return TypedArrayPrototypeGetBuffer(cipherText);
      }
      case "AES-GCM":
      case "AES-OCB": {
        normalizedAlgorithm.iv = copyBuffer(normalizedAlgorithm.iv);

        // 1.
        if (normalizedAlgorithm.tagLength === undefined) {
          normalizedAlgorithm.tagLength = 128;
        } else if (
          !ArrayPrototypeIncludes(
            [32, 64, 96, 104, 112, 120, 128],
            normalizedAlgorithm.tagLength,
          )
        ) {
          throw new DOMException(
            `Invalid tag length: ${normalizedAlgorithm.tagLength}`,
            "OperationError",
          );
        }

        // 2.
        if (
          TypedArrayPrototypeGetByteLength(data) <
            normalizedAlgorithm.tagLength / 8
        ) {
          throw new DOMException(
            "The provided data is too small",
            "OperationError",
          );
        }

        // 3. We only support 96-bit and 128-bit nonce for GCM, 1-15 bytes for OCB
        const ivLen = TypedArrayPrototypeGetByteLength(normalizedAlgorithm.iv);
        if (algorithm.name === "AES-GCM") {
          if (!ArrayPrototypeIncludes([12, 16], ivLen)) {
            throw new DOMException(
              "Initialization vector length not supported",
              "NotSupportedError",
            );
          }
        } else { // AES-OCB
          if (ivLen < 1 || ivLen > 15) {
            throw new DOMException(
              "Initialization vector length not supported",
              "NotSupportedError",
            );
          }
        }

        // 4.
        if (normalizedAlgorithm.additionalData !== undefined) {
          // NOTE: over the size of Number.MAX_SAFE_INTEGER is not available in V8
          // if (normalizedAlgorithm.additionalData.byteLength > (2 ** 64) - 1) {
          //   throw new DOMException(
          //     "Additional data too large",
          //     "OperationError",
          //   );
          // }
          normalizedAlgorithm.additionalData = copyBuffer(
            normalizedAlgorithm.additionalData,
          );
        }

        // 5-8.
        const plaintext = await op_crypto_decrypt(handle.cppgc, {
          algorithm: algorithm.name,
          length: key[_algorithm].length,
          iv: normalizedAlgorithm.iv,
          additionalData: normalizedAlgorithm.additionalData ||
            null,
          tagLength: normalizedAlgorithm.tagLength,
        }, data);

        // 9.
        return TypedArrayPrototypeGetBuffer(plaintext);
      }
      case "ChaCha20-Poly1305": {
        if (normalizedAlgorithm.iv === undefined) {
          throw new TypeError("iv is required");
        }
        normalizedAlgorithm.iv = copyBuffer(normalizedAlgorithm.iv);
        if (
          TypedArrayPrototypeGetByteLength(normalizedAlgorithm.iv) !== 12
        ) {
          throw new DOMException(
            "ChaCha20-Poly1305 iv must be 12 bytes",
            "OperationError",
          );
        }
        if (TypedArrayPrototypeGetByteLength(data) < 16) {
          throw new DOMException(
            "The provided data is too small",
            "OperationError",
          );
        }
        if (normalizedAlgorithm.additionalData !== undefined) {
          normalizedAlgorithm.additionalData = copyBuffer(
            normalizedAlgorithm.additionalData,
          );
        }

        const plaintext = await op_crypto_decrypt(handle.cppgc, {
          algorithm: "ChaCha20-Poly1305",
          nonce: normalizedAlgorithm.iv,
          additionalData: normalizedAlgorithm.additionalData || null,
        }, data);

        return TypedArrayPrototypeGetBuffer(plaintext);
      }
      default:
        throw new DOMException("Not implemented", "NotSupportedError");
    }
  }

  /**
   * @param {string} algorithm
   * @param {CryptoKey} key
   * @param {BufferSource} data
   * @returns {Promise<any>}
   */
  async sign(algorithm, key, data) {
    webidl.assertBranded(this, SubtleCryptoPrototype);
    const prefix = "Failed to execute 'sign' on 'SubtleCrypto'";
    webidl.requiredArguments(arguments.length, 3, prefix);
    algorithm = webidl.converters.AlgorithmIdentifier(
      algorithm,
      prefix,
      "Argument 1",
    );
    key = webidl.converters.CryptoKey(key, prefix, "Argument 2");
    data = webidl.converters.BufferSource(data, prefix, "Argument 3");

    // 1.
    data = copyBuffer(data);

    // 2.
    const normalizedAlgorithm = normalizeAlgorithm(algorithm, "sign");

    const handle = key[_handle];

    // 8.
    if (normalizedAlgorithm.name !== key[_algorithm].name) {
      throw new DOMException(
        "Signing algorithm does not match key algorithm",
        "InvalidAccessError",
      );
    }

    // 9.
    if (!ArrayPrototypeIncludes(key[_usages], "sign")) {
      throw new DOMException(
        "The requested operation is not valid for the provided key",
        "InvalidAccessError",
      );
    }

    switch (normalizedAlgorithm.name) {
      case "RSASSA-PKCS1-v1_5": {
        // 1.
        if (key[_type] !== "private") {
          throw new DOMException(
            "Key type not supported",
            "InvalidAccessError",
          );
        }

        // 2.
        const hashAlgorithm = key[_algorithm].hash.name;
        const signature = await op_crypto_sign_key(handle.cppgc, {
          algorithm: "RSASSA-PKCS1-v1_5",
          hash: hashAlgorithm,
        }, data);

        return TypedArrayPrototypeGetBuffer(signature);
      }
      case "RSA-PSS": {
        // 1.
        if (key[_type] !== "private") {
          throw new DOMException(
            "Key type not supported",
            "InvalidAccessError",
          );
        }

        // 2.
        const hashAlgorithm = key[_algorithm].hash.name;
        const signature = await op_crypto_sign_key(handle.cppgc, {
          algorithm: "RSA-PSS",
          hash: hashAlgorithm,
          saltLength: normalizedAlgorithm.saltLength,
        }, data);

        return TypedArrayPrototypeGetBuffer(signature);
      }
      case "ECDSA": {
        // 1.
        if (key[_type] !== "private") {
          throw new DOMException(
            "Key type not supported",
            "InvalidAccessError",
          );
        }

        // 2.
        const hashAlgorithm = normalizedAlgorithm.hash.name;
        const namedCurve = key[_algorithm].namedCurve;
        if (!ArrayPrototypeIncludes(supportedNamedCurves, namedCurve)) {
          throw new DOMException("Curve not supported", "NotSupportedError");
        }

        const signature = await op_crypto_sign_key(handle.cppgc, {
          algorithm: "ECDSA",
          hash: hashAlgorithm,
          namedCurve,
        }, data);

        return TypedArrayPrototypeGetBuffer(signature);
      }
      case "HMAC": {
        const hashAlgorithm = key[_algorithm].hash.name;

        const signature = await op_crypto_sign_key(handle.cppgc, {
          algorithm: "HMAC",
          hash: hashAlgorithm,
        }, data);

        return TypedArrayPrototypeGetBuffer(signature);
      }
      case "Ed25519": {
        // 1.
        if (key[_type] !== "private") {
          throw new DOMException(
            "Key type not supported",
            "InvalidAccessError",
          );
        }

        // https://briansmith.org/rustdoc/src/ring/ec/curve25519/ed25519/signing.rs.html#260
        const SIGNATURE_LEN = 32 * 2; // ELEM_LEN + SCALAR_LEN
        const signature = new Uint8Array(SIGNATURE_LEN);
        if (!op_crypto_sign_ed25519(handle.cppgc, data, signature)) {
          throw new DOMException(
            "Failed to sign",
            "OperationError",
          );
        }
        return TypedArrayPrototypeGetBuffer(signature);
      }
      case "ML-DSA-44":
      case "ML-DSA-65":
      case "ML-DSA-87": {
        if (key[_type] !== "private") {
          throw new DOMException(
            "Key type not supported",
            "InvalidAccessError",
          );
        }
        const variant = mldsaVariantId(normalizedAlgorithm.name);
        const context = normalizedAlgorithm.context;
        const signature = op_crypto_sign_mldsa(
          variant,
          handle.cppgc,
          data,
          context !== undefined ? context : null,
        );
        return TypedArrayPrototypeGetBuffer(signature);
      }
    }

    throw new TypeError("Unreachable");
  }

  /**
   * @param {string} format
   * @param {BufferSource} keyData
   * @param {string} algorithm
   * @param {boolean} extractable
   * @param {KeyUsages[]} keyUsages
   * @returns {Promise<any>}
   */
  async importKey(format, keyData, algorithm, extractable, keyUsages) {
    webidl.assertBranded(this, SubtleCryptoPrototype);
    const prefix = "Failed to execute 'importKey' on 'SubtleCrypto'";
    webidl.requiredArguments(arguments.length, 4, prefix);
    format = webidl.converters.KeyFormat(format, prefix, "Argument 1");
    keyData = webidl.converters["BufferSource or JsonWebKey"](
      keyData,
      prefix,
      "Argument 2",
    );
    algorithm = webidl.converters.AlgorithmIdentifier(
      algorithm,
      prefix,
      "Argument 3",
    );
    extractable = webidl.converters.boolean(extractable, prefix, "Argument 4");
    keyUsages = webidl.converters["sequence<KeyUsage>"](
      keyUsages,
      prefix,
      "Argument 5",
    );

    // 2.
    if (format !== "jwk") {
      if (ArrayBufferIsView(keyData) || isArrayBuffer(keyData)) {
        keyData = copyBuffer(keyData);
      } else {
        throw new TypeError("Cannot import key: 'keyData' is a JsonWebKey");
      }
    } else {
      if (ArrayBufferIsView(keyData) || isArrayBuffer(keyData)) {
        throw new TypeError("Cannot import key: 'keyData' is not a JsonWebKey");
      }
    }

    const normalizedAlgorithm = normalizeAlgorithm(algorithm, "importKey");

    // 8.
    const result = await importKeyInner(
      format,
      normalizedAlgorithm,
      keyData,
      extractable,
      keyUsages,
    );

    // 9.
    if (
      ArrayPrototypeIncludes(["private", "secret"], result[_type]) &&
      keyUsages.length == 0
    ) {
      throw new SyntaxError("Invalid key usage");
    }

    return result;
  }

  /**
   * @param {string} format
   * @param {CryptoKey} key
   * @returns {Promise<any>}
   */
  // deno-lint-ignore require-await
  async exportKey(format, key) {
    webidl.assertBranded(this, SubtleCryptoPrototype);
    const prefix = "Failed to execute 'exportKey' on 'SubtleCrypto'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    format = webidl.converters.KeyFormat(format, prefix, "Argument 1");
    key = webidl.converters.CryptoKey(key, prefix, "Argument 2");

    const handle = key[_handle];
    // 2.
    const innerKey = getKeyData(handle);

    const algorithmName = key[_algorithm].name;

    let result;

    switch (algorithmName) {
      case "HMAC": {
        result = exportKeyHMAC(format, key, innerKey);
        break;
      }
      case "RSASSA-PKCS1-v1_5":
      case "RSA-PSS":
      case "RSA-OAEP": {
        result = exportKeyRSA(format, key, innerKey);
        break;
      }
      case "ECDH":
      case "ECDSA": {
        result = exportKeyEC(format, key, innerKey);
        break;
      }
      case "Ed25519": {
        result = exportKeyEd25519(format, key, innerKey);
        break;
      }
      case "ML-DSA-44":
      case "ML-DSA-65":
      case "ML-DSA-87": {
        result = exportKeyMlDsa(format, key, innerKey);
        break;
      }
      case "X448": {
        result = exportKeyX448(format, key, innerKey);
        break;
      }
      case "X25519": {
        result = exportKeyX25519(format, key, innerKey);
        break;
      }
      case "AES-CTR":
      case "AES-CBC":
      case "AES-GCM":
      case "AES-OCB":
      case "AES-KW": {
        result = exportKeyAES(format, key, innerKey);
        break;
      }
      case "ChaCha20-Poly1305": {
        result = exportKeyChaCha20Poly1305(format, key, innerKey);
        break;
      }
      case "ML-KEM-512":
      case "ML-KEM-768":
      case "ML-KEM-1024": {
        result = exportKeyMlKem(format, key, innerKey);
        break;
      }
      default:
        throw new DOMException("Not implemented", "NotSupportedError");
    }

    if (key.extractable === false) {
      throw new DOMException(
        "Key is not extractable",
        "InvalidAccessError",
      );
    }

    return result;
  }

  /**
   * @param {AlgorithmIdentifier} algorithm
   * @param {CryptoKey} baseKey
   * @param {number | null} length
   * @returns {Promise<ArrayBuffer>}
   */
  async deriveBits(algorithm, baseKey, length = null) {
    webidl.assertBranded(this, SubtleCryptoPrototype);
    const prefix = "Failed to execute 'deriveBits' on 'SubtleCrypto'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    algorithm = webidl.converters.AlgorithmIdentifier(
      algorithm,
      prefix,
      "Argument 1",
    );
    baseKey = webidl.converters.CryptoKey(baseKey, prefix, "Argument 2");
    if (length !== null) {
      length = webidl.converters["unsigned long"](length, prefix, "Argument 3");
    }

    // 2.
    const normalizedAlgorithm = normalizeAlgorithm(algorithm, "deriveBits");
    // 4-6.
    const result = await deriveBits(normalizedAlgorithm, baseKey, length);
    // 7.
    if (normalizedAlgorithm.name !== baseKey[_algorithm].name) {
      throw new DOMException("Invalid algorithm name", "InvalidAccessError");
    }
    // 8.
    if (!ArrayPrototypeIncludes(baseKey[_usages], "deriveBits")) {
      throw new DOMException(
        "'baseKey' usages does not contain 'deriveBits'",
        "InvalidAccessError",
      );
    }
    // 9-10.
    return result;
  }

  /**
   * @param {AlgorithmIdentifier} algorithm
   * @param {CryptoKey} baseKey
   * @param {number} length
   * @returns {Promise<ArrayBuffer>}
   */
  async deriveKey(
    algorithm,
    baseKey,
    derivedKeyType,
    extractable,
    keyUsages,
  ) {
    webidl.assertBranded(this, SubtleCryptoPrototype);
    const prefix = "Failed to execute 'deriveKey' on 'SubtleCrypto'";
    webidl.requiredArguments(arguments.length, 5, prefix);
    algorithm = webidl.converters.AlgorithmIdentifier(
      algorithm,
      prefix,
      "Argument 1",
    );
    baseKey = webidl.converters.CryptoKey(baseKey, prefix, "Argument 2");
    derivedKeyType = webidl.converters.AlgorithmIdentifier(
      derivedKeyType,
      prefix,
      "Argument 3",
    );
    extractable = webidl.converters["boolean"](
      extractable,
      prefix,
      "Argument 4",
    );
    keyUsages = webidl.converters["sequence<KeyUsage>"](
      keyUsages,
      prefix,
      "Argument 5",
    );

    // 2-3.
    const normalizedAlgorithm = normalizeAlgorithm(algorithm, "deriveBits");

    // 4-5.
    const normalizedDerivedKeyAlgorithmImport = normalizeAlgorithm(
      derivedKeyType,
      "importKey",
    );

    // 6-7.
    const normalizedDerivedKeyAlgorithmLength = normalizeAlgorithm(
      derivedKeyType,
      "get key length",
    );

    // 8-10.

    // 11.
    if (normalizedAlgorithm.name !== baseKey[_algorithm].name) {
      throw new DOMException(
        `Invalid algorithm name: ${normalizedAlgorithm.name}`,
        "InvalidAccessError",
      );
    }

    // 12.
    if (!ArrayPrototypeIncludes(baseKey[_usages], "deriveKey")) {
      throw new DOMException(
        "'baseKey' usages does not contain 'deriveKey'",
        "InvalidAccessError",
      );
    }

    // 13.
    const length = getKeyLength(normalizedDerivedKeyAlgorithmLength);

    // 14.
    const secret = await deriveBits(
      normalizedAlgorithm,
      baseKey,
      length,
    );

    // 15.
    // Use "raw-secret" (the unified symmetric key format) so deriveKey works
    // for both the existing symmetric algorithms (where "raw" is an alias) and
    // the modern ones (e.g. ChaCha20-Poly1305) that only accept "raw-secret".
    const result = await this.importKey(
      "raw-secret",
      secret,
      normalizedDerivedKeyAlgorithmImport,
      extractable,
      keyUsages,
    );

    // 16.
    if (
      ArrayPrototypeIncludes(["private", "secret"], result[_type]) &&
      keyUsages.length == 0
    ) {
      throw new SyntaxError("Invalid key usage");
    }
    // 17.
    return result;
  }

  /**
   * @param {string} algorithm
   * @param {CryptoKey} key
   * @param {BufferSource} signature
   * @param {BufferSource} data
   * @returns {Promise<boolean>}
   */
  async verify(algorithm, key, signature, data) {
    webidl.assertBranded(this, SubtleCryptoPrototype);
    const prefix = "Failed to execute 'verify' on 'SubtleCrypto'";
    webidl.requiredArguments(arguments.length, 4, prefix);
    algorithm = webidl.converters.AlgorithmIdentifier(
      algorithm,
      prefix,
      "Argument 1",
    );
    key = webidl.converters.CryptoKey(key, prefix, "Argument 2");
    signature = webidl.converters.BufferSource(signature, prefix, "Argument 3");
    data = webidl.converters.BufferSource(data, prefix, "Argument 4");

    // 2.
    signature = copyBuffer(signature);

    // 3.
    data = copyBuffer(data);

    const normalizedAlgorithm = normalizeAlgorithm(algorithm, "verify");

    const handle = key[_handle];

    if (normalizedAlgorithm.name !== key[_algorithm].name) {
      throw new DOMException(
        "Verifying algorithm does not match key algorithm",
        "InvalidAccessError",
      );
    }

    if (!ArrayPrototypeIncludes(key[_usages], "verify")) {
      throw new DOMException(
        "The requested operation is not valid for the provided key",
        "InvalidAccessError",
      );
    }

    switch (normalizedAlgorithm.name) {
      case "RSASSA-PKCS1-v1_5": {
        if (key[_type] !== "public") {
          throw new DOMException(
            "Key type not supported",
            "InvalidAccessError",
          );
        }

        const hashAlgorithm = key[_algorithm].hash.name;
        return await op_crypto_verify_key(handle.cppgc, {
          algorithm: "RSASSA-PKCS1-v1_5",
          hash: hashAlgorithm,
          signature,
        }, data);
      }
      case "RSA-PSS": {
        if (key[_type] !== "public") {
          throw new DOMException(
            "Key type not supported",
            "InvalidAccessError",
          );
        }

        const hashAlgorithm = key[_algorithm].hash.name;
        return await op_crypto_verify_key(handle.cppgc, {
          algorithm: "RSA-PSS",
          hash: hashAlgorithm,
          signature,
          saltLength: normalizedAlgorithm.saltLength,
        }, data);
      }
      case "HMAC": {
        const hash = key[_algorithm].hash.name;
        return await op_crypto_verify_key(handle.cppgc, {
          algorithm: "HMAC",
          hash,
          signature,
        }, data);
      }
      case "ECDSA": {
        // 1.
        if (key[_type] !== "public") {
          throw new DOMException(
            "Key type not supported",
            "InvalidAccessError",
          );
        }
        // 2.
        const hash = normalizedAlgorithm.hash.name;
        // 3-8.
        return await op_crypto_verify_key(handle.cppgc, {
          algorithm: "ECDSA",
          hash,
          signature,
          namedCurve: key[_algorithm].namedCurve,
        }, data);
      }
      case "Ed25519": {
        // 1.
        if (key[_type] !== "public") {
          throw new DOMException(
            "Key type not supported",
            "InvalidAccessError",
          );
        }

        return op_crypto_verify_ed25519(handle.cppgc, data, signature);
      }
      case "ML-DSA-44":
      case "ML-DSA-65":
      case "ML-DSA-87": {
        if (key[_type] !== "public") {
          throw new DOMException(
            "Key type not supported",
            "InvalidAccessError",
          );
        }
        const variant = mldsaVariantId(normalizedAlgorithm.name);
        const context = normalizedAlgorithm.context;
        return op_crypto_verify_mldsa(
          variant,
          handle.cppgc,
          data,
          signature,
          context !== undefined ? context : null,
        );
      }
    }

    throw new TypeError("Unreachable");
  }

  /**
   * @param {string} algorithm
   * @param {boolean} extractable
   * @param {KeyUsage[]} keyUsages
   * @returns {Promise<any>}
   */
  async wrapKey(format, key, wrappingKey, wrapAlgorithm) {
    webidl.assertBranded(this, SubtleCryptoPrototype);
    const prefix = "Failed to execute 'wrapKey' on 'SubtleCrypto'";
    webidl.requiredArguments(arguments.length, 4, prefix);
    format = webidl.converters.KeyFormat(format, prefix, "Argument 1");
    key = webidl.converters.CryptoKey(key, prefix, "Argument 2");
    wrappingKey = webidl.converters.CryptoKey(
      wrappingKey,
      prefix,
      "Argument 3",
    );
    wrapAlgorithm = webidl.converters.AlgorithmIdentifier(
      wrapAlgorithm,
      prefix,
      "Argument 4",
    );

    let normalizedAlgorithm;

    try {
      // 2.
      normalizedAlgorithm = normalizeAlgorithm(wrapAlgorithm, "wrapKey");
    } catch (_) {
      // 3.
      normalizedAlgorithm = normalizeAlgorithm(wrapAlgorithm, "encrypt");
    }

    // 8.
    if (normalizedAlgorithm.name !== wrappingKey[_algorithm].name) {
      throw new DOMException(
        "Wrapping algorithm does not match key algorithm",
        "InvalidAccessError",
      );
    }

    // 9.
    if (!ArrayPrototypeIncludes(wrappingKey[_usages], "wrapKey")) {
      throw new DOMException(
        "The requested operation is not valid for the provided key",
        "InvalidAccessError",
      );
    }

    // 10. NotSupportedError will be thrown in step 12.
    // 11.
    if (key[_extractable] === false) {
      throw new DOMException(
        "Key is not extractable",
        "InvalidAccessError",
      );
    }

    // 12.
    const exportedKey = await this.exportKey(format, key);

    let bytes;
    // 13.
    if (format !== "jwk") {
      bytes = new Uint8Array(exportedKey);
    } else {
      const jwk = JSONStringify(exportedKey);
      const ret = new Uint8Array(jwk.length);
      for (let i = 0; i < jwk.length; i++) {
        ret[i] = StringPrototypeCharCodeAt(jwk, i);
      }
      bytes = ret;
    }

    // 14-15.
    if (
      supportedAlgorithms["wrapKey"][normalizedAlgorithm.name] !== undefined
    ) {
      const handle = wrappingKey[_handle];

      switch (normalizedAlgorithm.name) {
        case "AES-KW": {
          const cipherText = await op_crypto_wrap_key(handle.cppgc, {
            algorithm: normalizedAlgorithm.name,
          }, bytes);

          // 4.
          return TypedArrayPrototypeGetBuffer(cipherText);
        }
        default: {
          throw new DOMException(
            "Not implemented",
            "NotSupportedError",
          );
        }
      }
    } else if (
      supportedAlgorithms["encrypt"][normalizedAlgorithm.name] !== undefined
    ) {
      // must construct a new key, since keyUsages is ["wrapKey"] and not ["encrypt"]
      return await encrypt(
        normalizedAlgorithm,
        constructKey(
          wrappingKey[_type],
          wrappingKey[_extractable],
          ["encrypt"],
          wrappingKey[_algorithm],
          wrappingKey[_handle],
        ),
        bytes,
      );
    } else {
      throw new DOMException(
        "Algorithm not supported",
        "NotSupportedError",
      );
    }
  }
  /**
   * @param {string} format
   * @param {BufferSource} wrappedKey
   * @param {CryptoKey} unwrappingKey
   * @param {AlgorithmIdentifier} unwrapAlgorithm
   * @param {AlgorithmIdentifier} unwrappedKeyAlgorithm
   * @param {boolean} extractable
   * @param {KeyUsage[]} keyUsages
   * @returns {Promise<CryptoKey>}
   */
  async unwrapKey(
    format,
    wrappedKey,
    unwrappingKey,
    unwrapAlgorithm,
    unwrappedKeyAlgorithm,
    extractable,
    keyUsages,
  ) {
    webidl.assertBranded(this, SubtleCryptoPrototype);
    const prefix = "Failed to execute 'unwrapKey' on 'SubtleCrypto'";
    webidl.requiredArguments(arguments.length, 7, prefix);
    format = webidl.converters.KeyFormat(format, prefix, "Argument 1");
    wrappedKey = webidl.converters.BufferSource(
      wrappedKey,
      prefix,
      "Argument 2",
    );
    unwrappingKey = webidl.converters.CryptoKey(
      unwrappingKey,
      prefix,
      "Argument 3",
    );
    unwrapAlgorithm = webidl.converters.AlgorithmIdentifier(
      unwrapAlgorithm,
      prefix,
      "Argument 4",
    );
    unwrappedKeyAlgorithm = webidl.converters.AlgorithmIdentifier(
      unwrappedKeyAlgorithm,
      prefix,
      "Argument 5",
    );
    extractable = webidl.converters.boolean(extractable, prefix, "Argument 6");
    keyUsages = webidl.converters["sequence<KeyUsage>"](
      keyUsages,
      prefix,
      "Argument 7",
    );

    // 2.
    wrappedKey = copyBuffer(wrappedKey);

    let normalizedAlgorithm;

    try {
      // 3.
      normalizedAlgorithm = normalizeAlgorithm(unwrapAlgorithm, "unwrapKey");
    } catch (_) {
      // 4.
      normalizedAlgorithm = normalizeAlgorithm(unwrapAlgorithm, "decrypt");
    }

    // 6.
    const normalizedKeyAlgorithm = normalizeAlgorithm(
      unwrappedKeyAlgorithm,
      "importKey",
    );

    // 11.
    if (normalizedAlgorithm.name !== unwrappingKey[_algorithm].name) {
      throw new DOMException(
        "Unwrapping algorithm does not match key algorithm",
        "InvalidAccessError",
      );
    }

    // 12.
    if (!ArrayPrototypeIncludes(unwrappingKey[_usages], "unwrapKey")) {
      throw new DOMException(
        "The requested operation is not valid for the provided key",
        "InvalidAccessError",
      );
    }

    // 13.
    let key;
    if (
      supportedAlgorithms["unwrapKey"][normalizedAlgorithm.name] !== undefined
    ) {
      const handle = unwrappingKey[_handle];

      switch (normalizedAlgorithm.name) {
        case "AES-KW": {
          const plainText = await op_crypto_unwrap_key(handle.cppgc, {
            algorithm: normalizedAlgorithm.name,
          }, wrappedKey);

          // 4.
          key = TypedArrayPrototypeGetBuffer(plainText);
          break;
        }
        default: {
          throw new DOMException(
            "Not implemented",
            "NotSupportedError",
          );
        }
      }
    } else if (
      supportedAlgorithms["decrypt"][normalizedAlgorithm.name] !== undefined
    ) {
      // must construct a new key, since keyUsages is ["unwrapKey"] and not ["decrypt"]
      key = await this.decrypt(
        normalizedAlgorithm,
        constructKey(
          unwrappingKey[_type],
          unwrappingKey[_extractable],
          ["decrypt"],
          unwrappingKey[_algorithm],
          unwrappingKey[_handle],
        ),
        wrappedKey,
      );
    } else {
      throw new DOMException(
        "Algorithm not supported",
        "NotSupportedError",
      );
    }

    let bytes;
    // 14.
    if (format !== "jwk") {
      bytes = key;
    } else {
      const k = new Uint8Array(key);
      let str = "";
      for (let i = 0; i < k.length; i++) {
        str += StringFromCharCode(k[i]);
      }
      bytes = JSONParse(str);
    }

    // 15.
    const result = await this.importKey(
      format,
      bytes,
      normalizedKeyAlgorithm,
      extractable,
      keyUsages,
    );
    // 16.
    if (
      (result[_type] == "secret" || result[_type] == "private") &&
      keyUsages.length == 0
    ) {
      throw new SyntaxError("Invalid key type");
    }
    // 17.
    result[_extractable] = extractable;
    // 18.
    result[_usages] = usageIntersection(keyUsages, recognisedUsages);
    // 19.
    return result;
  }

  /**
   * @param {string} algorithm
   * @param {boolean} extractable
   * @param {KeyUsage[]} keyUsages
   * @returns {Promise<any>}
   */
  async generateKey(algorithm, extractable, keyUsages) {
    webidl.assertBranded(this, SubtleCryptoPrototype);
    const prefix = "Failed to execute 'generateKey' on 'SubtleCrypto'";
    webidl.requiredArguments(arguments.length, 3, prefix);
    algorithm = webidl.converters.AlgorithmIdentifier(
      algorithm,
      prefix,
      "Argument 1",
    );
    extractable = webidl.converters["boolean"](
      extractable,
      prefix,
      "Argument 2",
    );
    keyUsages = webidl.converters["sequence<KeyUsage>"](
      keyUsages,
      prefix,
      "Argument 3",
    );

    const usages = keyUsages;

    const normalizedAlgorithm = normalizeAlgorithm(algorithm, "generateKey");
    const result = await generateKey(
      normalizedAlgorithm,
      extractable,
      usages,
    );

    if (ObjectPrototypeIsPrototypeOf(CryptoKeyPrototype, result)) {
      const type = result[_type];
      if ((type === "secret" || type === "private") && usages.length === 0) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }
    } else if (
      ObjectPrototypeIsPrototypeOf(CryptoKeyPrototype, result.privateKey)
    ) {
      if (result.privateKey[_usages].length === 0) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }
    }

    return result;
  }

  /**
   * Encapsulate a fresh shared secret to the given encapsulation key and
   * return the shared secret as a CryptoKey, along with the ciphertext.
   *
   * https://wicg.github.io/webcrypto-modern-algos/#SubtleCrypto-method-encapsulateKey
   *
   * @param {AlgorithmIdentifier} algorithm
   * @param {CryptoKey} encapsulationKey
   * @param {AlgorithmIdentifier} sharedKeyAlgorithm
   * @param {boolean} extractable
   * @param {KeyUsage[]} usages
   * @returns {Promise<{ciphertext: ArrayBuffer, sharedKey: CryptoKey}>}
   */
  async encapsulateKey(
    algorithm,
    encapsulationKey,
    sharedKeyAlgorithm,
    extractable,
    usages,
  ) {
    webidl.assertBranded(this, SubtleCryptoPrototype);
    const prefix = "Failed to execute 'encapsulateKey' on 'SubtleCrypto'";
    webidl.requiredArguments(arguments.length, 5, prefix);
    algorithm = webidl.converters.AlgorithmIdentifier(
      algorithm,
      prefix,
      "Argument 1",
    );
    encapsulationKey = webidl.converters.CryptoKey(
      encapsulationKey,
      prefix,
      "Argument 2",
    );
    sharedKeyAlgorithm = webidl.converters.AlgorithmIdentifier(
      sharedKeyAlgorithm,
      prefix,
      "Argument 3",
    );
    extractable = webidl.converters.boolean(extractable, prefix, "Argument 4");
    usages = webidl.converters["sequence<KeyUsage>"](
      usages,
      prefix,
      "Argument 5",
    );

    const normalizedAlgorithm = normalizeAlgorithm(algorithm, "encapsulate");

    if (encapsulationKey[_algorithm].name !== normalizedAlgorithm.name) {
      throw new DOMException(
        "Encapsulation key algorithm does not match",
        "InvalidAccessError",
      );
    }
    if (encapsulationKey[_type] !== "public") {
      throw new DOMException(
        "Encapsulation key must be a public key",
        "InvalidAccessError",
      );
    }
    if (!ArrayPrototypeIncludes(encapsulationKey[_usages], "encapsulateKey")) {
      throw new DOMException(
        "Encapsulation key usages must include 'encapsulateKey'",
        "InvalidAccessError",
      );
    }

    const { ciphertext, sharedSecret } = mlKemEncapsulate(
      normalizedAlgorithm,
      encapsulationKey,
    );

    const sharedKey = await this.importKey(
      "raw",
      sharedSecret,
      sharedKeyAlgorithm,
      extractable,
      usages,
    );

    return {
      ciphertext: TypedArrayPrototypeGetBuffer(ciphertext),
      sharedKey,
    };
  }

  /**
   * Encapsulate a fresh shared secret to the given encapsulation key and
   * return the raw shared secret bytes.
   *
   * https://wicg.github.io/webcrypto-modern-algos/#SubtleCrypto-method-encapsulateBits
   *
   * @param {AlgorithmIdentifier} algorithm
   * @param {CryptoKey} encapsulationKey
   * @returns {Promise<{ciphertext: ArrayBuffer, sharedKey: ArrayBuffer}>}
   */
  // deno-lint-ignore require-await
  async encapsulateBits(algorithm, encapsulationKey) {
    webidl.assertBranded(this, SubtleCryptoPrototype);
    const prefix = "Failed to execute 'encapsulateBits' on 'SubtleCrypto'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    algorithm = webidl.converters.AlgorithmIdentifier(
      algorithm,
      prefix,
      "Argument 1",
    );
    encapsulationKey = webidl.converters.CryptoKey(
      encapsulationKey,
      prefix,
      "Argument 2",
    );

    const normalizedAlgorithm = normalizeAlgorithm(algorithm, "encapsulate");

    if (encapsulationKey[_algorithm].name !== normalizedAlgorithm.name) {
      throw new DOMException(
        "Encapsulation key algorithm does not match",
        "InvalidAccessError",
      );
    }
    if (encapsulationKey[_type] !== "public") {
      throw new DOMException(
        "Encapsulation key must be a public key",
        "InvalidAccessError",
      );
    }
    if (!ArrayPrototypeIncludes(encapsulationKey[_usages], "encapsulateBits")) {
      throw new DOMException(
        "Encapsulation key usages must include 'encapsulateBits'",
        "InvalidAccessError",
      );
    }

    const { ciphertext, sharedSecret } = mlKemEncapsulate(
      normalizedAlgorithm,
      encapsulationKey,
    );
    return {
      ciphertext: TypedArrayPrototypeGetBuffer(ciphertext),
      sharedKey: TypedArrayPrototypeGetBuffer(sharedSecret),
    };
  }

  /**
   * Decapsulate the given ciphertext using the provided decapsulation key,
   * importing the resulting shared secret as a CryptoKey under
   * `sharedKeyAlgorithm`.
   *
   * https://wicg.github.io/webcrypto-modern-algos/#SubtleCrypto-method-decapsulateKey
   *
   * @param {AlgorithmIdentifier} algorithm
   * @param {CryptoKey} decapsulationKey
   * @param {BufferSource} ciphertext
   * @param {AlgorithmIdentifier} sharedKeyAlgorithm
   * @param {boolean} extractable
   * @param {KeyUsage[]} usages
   * @returns {Promise<CryptoKey>}
   */
  async decapsulateKey(
    algorithm,
    decapsulationKey,
    ciphertext,
    sharedKeyAlgorithm,
    extractable,
    usages,
  ) {
    webidl.assertBranded(this, SubtleCryptoPrototype);
    const prefix = "Failed to execute 'decapsulateKey' on 'SubtleCrypto'";
    webidl.requiredArguments(arguments.length, 6, prefix);
    algorithm = webidl.converters.AlgorithmIdentifier(
      algorithm,
      prefix,
      "Argument 1",
    );
    decapsulationKey = webidl.converters.CryptoKey(
      decapsulationKey,
      prefix,
      "Argument 2",
    );
    ciphertext = webidl.converters.BufferSource(
      ciphertext,
      prefix,
      "Argument 3",
    );
    sharedKeyAlgorithm = webidl.converters.AlgorithmIdentifier(
      sharedKeyAlgorithm,
      prefix,
      "Argument 4",
    );
    extractable = webidl.converters.boolean(extractable, prefix, "Argument 5");
    usages = webidl.converters["sequence<KeyUsage>"](
      usages,
      prefix,
      "Argument 6",
    );

    ciphertext = copyBuffer(ciphertext);
    const normalizedAlgorithm = normalizeAlgorithm(algorithm, "decapsulate");
    if (decapsulationKey[_algorithm].name !== normalizedAlgorithm.name) {
      throw new DOMException(
        "Decapsulation key algorithm does not match",
        "InvalidAccessError",
      );
    }
    if (decapsulationKey[_type] !== "private") {
      throw new DOMException(
        "Decapsulation key must be a private key",
        "InvalidAccessError",
      );
    }
    if (!ArrayPrototypeIncludes(decapsulationKey[_usages], "decapsulateKey")) {
      throw new DOMException(
        "Decapsulation key usages must include 'decapsulateKey'",
        "InvalidAccessError",
      );
    }

    const sharedSecret = mlKemDecapsulate(
      normalizedAlgorithm,
      decapsulationKey,
      ciphertext,
    );

    return await this.importKey(
      "raw",
      sharedSecret,
      sharedKeyAlgorithm,
      extractable,
      usages,
    );
  }

  /**
   * Decapsulate the given ciphertext using the provided decapsulation key
   * and return the raw shared secret bytes.
   *
   * https://wicg.github.io/webcrypto-modern-algos/#SubtleCrypto-method-decapsulateBits
   *
   * @param {AlgorithmIdentifier} algorithm
   * @param {CryptoKey} decapsulationKey
   * @param {BufferSource} ciphertext
   * @returns {Promise<ArrayBuffer>}
   */
  // deno-lint-ignore require-await
  async decapsulateBits(algorithm, decapsulationKey, ciphertext) {
    webidl.assertBranded(this, SubtleCryptoPrototype);
    const prefix = "Failed to execute 'decapsulateBits' on 'SubtleCrypto'";
    webidl.requiredArguments(arguments.length, 3, prefix);
    algorithm = webidl.converters.AlgorithmIdentifier(
      algorithm,
      prefix,
      "Argument 1",
    );
    decapsulationKey = webidl.converters.CryptoKey(
      decapsulationKey,
      prefix,
      "Argument 2",
    );
    ciphertext = webidl.converters.BufferSource(
      ciphertext,
      prefix,
      "Argument 3",
    );

    ciphertext = copyBuffer(ciphertext);
    const normalizedAlgorithm = normalizeAlgorithm(algorithm, "decapsulate");
    if (decapsulationKey[_algorithm].name !== normalizedAlgorithm.name) {
      throw new DOMException(
        "Decapsulation key algorithm does not match",
        "InvalidAccessError",
      );
    }
    if (decapsulationKey[_type] !== "private") {
      throw new DOMException(
        "Decapsulation key must be a private key",
        "InvalidAccessError",
      );
    }
    if (!ArrayPrototypeIncludes(decapsulationKey[_usages], "decapsulateBits")) {
      throw new DOMException(
        "Decapsulation key usages must include 'decapsulateBits'",
        "InvalidAccessError",
      );
    }

    const sharedSecret = mlKemDecapsulate(
      normalizedAlgorithm,
      decapsulationKey,
      ciphertext,
    );
    return TypedArrayPrototypeGetBuffer(sharedSecret);
  }

  /**
   * Derive the public key associated with a private key, for asymmetric
   * algorithms (RSA, EC, Ed25519, X25519/X448 and the post-quantum ML-KEM and
   * ML-DSA families).
   *
   * https://wicg.github.io/webcrypto-modern-algos/#SubtleCrypto-method-getPublicKey
   *
   * @param {CryptoKey} key
   * @param {KeyUsage[]} keyUsages
   * @returns {Promise<CryptoKey>}
   */
  async getPublicKey(key, keyUsages) {
    webidl.assertBranded(this, SubtleCryptoPrototype);
    const prefix = "Failed to execute 'getPublicKey' on 'SubtleCrypto'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    key = webidl.converters.CryptoKey(key, prefix, "Argument 1");
    keyUsages = webidl.converters["sequence<KeyUsage>"](
      keyUsages,
      prefix,
      "Argument 2",
    );

    const algorithm = key[_algorithm];
    const algorithmName = algorithm.name;

    // 1. Algorithms that cannot derive a public key reject with a
    // NotSupportedError (this also covers symmetric and KDF algorithms).
    switch (algorithmName) {
      case "RSASSA-PKCS1-v1_5":
      case "RSA-PSS":
      case "RSA-OAEP":
      case "ECDSA":
      case "ECDH":
      case "Ed25519":
      case "X25519":
      case "X448":
      case "ML-DSA-44":
      case "ML-DSA-65":
      case "ML-DSA-87":
      case "ML-KEM-512":
      case "ML-KEM-768":
      case "ML-KEM-1024":
        break;
      default:
        throw new DOMException(
          `getPublicKey() is not supported for ${algorithmName}`,
          "NotSupportedError",
        );
    }

    // 2. The public key can only be derived from a private key.
    if (key[_type] !== "private") {
      throw new DOMException(
        "Public keys can only be derived from private keys",
        "InvalidAccessError",
      );
    }

    // 3. Derive the public key. For ML-KEM/ML-DSA the usages allowed for a
    // public key are validated here; for the other algorithms the derived
    // public key material is re-imported, which performs the same per-algorithm
    // usage validation (rejecting invalid usages with a SyntaxError).
    switch (algorithmName) {
      case "ML-KEM-512":
      case "ML-KEM-768":
      case "ML-KEM-1024": {
        validatePublicKeyUsages(keyUsages, ML_KEM_PUBLIC_USAGES);
        let publicKeyBytes;
        try {
          publicKeyBytes = op_crypto_ml_kem_get_public_key(
            algorithmName,
            key[_handle].cppgc,
          );
        } catch (_) {
          throw new DOMException(
            "Failed to derive public key",
            "OperationError",
          );
        }
        const pubHandle = {};
        setKeyData(pubHandle, publicKeyBytes);
        return constructKey(
          "public",
          true,
          keyUsages,
          { name: algorithmName },
          pubHandle,
        );
      }
      case "ML-DSA-44":
      case "ML-DSA-65":
      case "ML-DSA-87": {
        validatePublicKeyUsages(keyUsages, ["verify"]);
        // The matching public key is derived and stored alongside the private
        // key at generate/import time; reuse its key material.
        const pub = WeakMapPrototypeGet(MLDSA_PUBLIC_FROM_PRIVATE, key);
        if (pub === undefined) {
          throw new DOMException(
            "Failed to derive public key",
            "OperationError",
          );
        }
        return constructKey(
          "public",
          true,
          keyUsages,
          { name: algorithmName },
          pub[_handle],
        );
      }
      case "RSASSA-PKCS1-v1_5":
      case "RSA-PSS":
      case "RSA-OAEP": {
        let spki;
        try {
          spki = op_crypto_export_key({
            algorithm: algorithmName,
            format: "spki",
          }, getKeyData(key[_handle]));
        } catch (_) {
          throw new DOMException(
            "Failed to derive public key",
            "OperationError",
          );
        }
        return await this.importKey("spki", spki, algorithm, true, keyUsages);
      }
      case "ECDSA":
      case "ECDH": {
        let spki;
        try {
          spki = op_crypto_export_key({
            algorithm: algorithmName,
            namedCurve: algorithm.namedCurve,
            format: "spki",
          }, getKeyData(key[_handle]));
        } catch (_) {
          throw new DOMException(
            "Failed to derive public key",
            "OperationError",
          );
        }
        return await this.importKey("spki", spki, algorithm, true, keyUsages);
      }
      default: {
        // Ed25519, X25519 and X448 store raw key material; derive the raw
        // public key from the private key and re-import it as a JWK.
        let x;
        try {
          switch (algorithmName) {
            case "Ed25519":
              x = op_crypto_jwk_x_ed25519(getKeyData(key[_handle]));
              break;
            case "X25519":
              x = op_crypto_x25519_public_key(getKeyData(key[_handle]));
              break;
            default: // X448
              x = op_crypto_x448_public_key(getKeyData(key[_handle]));
              break;
          }
        } catch (_) {
          throw new DOMException(
            "Failed to derive public key",
            "OperationError",
          );
        }
        const jwk = {
          kty: "OKP",
          crv: algorithmName,
          x,
          ext: true,
        };
        return await this.importKey("jwk", jwk, algorithm, true, keyUsages);
      }
    }
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return `${this.constructor.name} ${inspect({}, inspectOptions)}`;
  }
}
const SubtleCryptoPrototype = SubtleCrypto.prototype;

function mlKemEncapsulate(normalizedAlgorithm, encapsulationKey) {
  switch (normalizedAlgorithm.name) {
    case "ML-KEM-512":
    case "ML-KEM-768":
    case "ML-KEM-1024": {
      const handle = encapsulationKey[_handle];
      let result;
      try {
        result = op_crypto_ml_kem_encapsulate(
          normalizedAlgorithm.name,
          handle.cppgc,
        );
      } catch (_) {
        throw new DOMException("Encapsulation failed", "OperationError");
      }
      return {
        ciphertext: result.ciphertext,
        sharedSecret: result.sharedSecret,
      };
    }
    default:
      throw new DOMException(
        `Encapsulation not supported for ${normalizedAlgorithm.name}`,
        "NotSupportedError",
      );
  }
}

function mlKemDecapsulate(normalizedAlgorithm, decapsulationKey, ciphertext) {
  switch (normalizedAlgorithm.name) {
    case "ML-KEM-512":
    case "ML-KEM-768":
    case "ML-KEM-1024": {
      const expectedCtSize = ML_KEM_CIPHERTEXT_SIZES[normalizedAlgorithm.name];
      if (TypedArrayPrototypeGetByteLength(ciphertext) !== expectedCtSize) {
        throw new DOMException(
          `ML-KEM ${normalizedAlgorithm.name} ciphertext must be ${expectedCtSize} bytes`,
          "OperationError",
        );
      }
      const handle = decapsulationKey[_handle];
      try {
        return op_crypto_ml_kem_decapsulate(
          normalizedAlgorithm.name,
          handle.cppgc,
          ciphertext,
        );
      } catch (_) {
        throw new DOMException("Decapsulation failed", "OperationError");
      }
    }
    default:
      throw new DOMException(
        `Decapsulation not supported for ${normalizedAlgorithm.name}`,
        "NotSupportedError",
      );
  }
}

async function generateKey(normalizedAlgorithm, extractable, usages) {
  const algorithmName = normalizedAlgorithm.name;

  switch (algorithmName) {
    case "RSASSA-PKCS1-v1_5":
    case "RSA-PSS": {
      // 1.
      if (
        ArrayPrototypeFind(
          usages,
          (u) => !ArrayPrototypeIncludes(["sign", "verify"], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      // 2.
      const keyData = await op_crypto_generate_key(
        {
          algorithm: "RSA",
          modulusLength: normalizedAlgorithm.modulusLength,
          publicExponent: normalizedAlgorithm.publicExponent,
        },
      );
      const handle = {};
      setKeyData(handle, {
        type: "private",
        data: keyData,
      });

      // 4-8.
      const algorithm = {
        name: algorithmName,
        modulusLength: normalizedAlgorithm.modulusLength,
        publicExponent: normalizedAlgorithm.publicExponent,
        hash: normalizedAlgorithm.hash,
      };

      // 9-13.
      const publicKey = constructKey(
        "public",
        true,
        usageIntersection(usages, ["verify"]),
        algorithm,
        handle,
      );

      // 14-18.
      const privateKey = constructKey(
        "private",
        extractable,
        usageIntersection(usages, ["sign"]),
        algorithm,
        handle,
      );

      // 19-22.
      return { publicKey, privateKey };
    }
    case "RSA-OAEP": {
      if (
        ArrayPrototypeFind(
          usages,
          (u) =>
            !ArrayPrototypeIncludes([
              "encrypt",
              "decrypt",
              "wrapKey",
              "unwrapKey",
            ], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      // 2.
      const keyData = await op_crypto_generate_key(
        {
          algorithm: "RSA",
          modulusLength: normalizedAlgorithm.modulusLength,
          publicExponent: normalizedAlgorithm.publicExponent,
        },
      );
      const handle = {};
      setKeyData(handle, {
        type: "private",
        data: keyData,
      });

      // 4-8.
      const algorithm = {
        name: algorithmName,
        modulusLength: normalizedAlgorithm.modulusLength,
        publicExponent: normalizedAlgorithm.publicExponent,
        hash: normalizedAlgorithm.hash,
      };

      // 9-13.
      const publicKey = constructKey(
        "public",
        true,
        usageIntersection(usages, ["encrypt", "wrapKey"]),
        algorithm,
        handle,
      );

      // 14-18.
      const privateKey = constructKey(
        "private",
        extractable,
        usageIntersection(usages, ["decrypt", "unwrapKey"]),
        algorithm,
        handle,
      );

      // 19-22.
      return { publicKey, privateKey };
    }
    case "ECDSA": {
      const namedCurve = normalizedAlgorithm.namedCurve;

      // 1.
      if (
        ArrayPrototypeFind(
          usages,
          (u) => !ArrayPrototypeIncludes(["sign", "verify"], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      // 2-3.
      const handle = {};
      if (
        ArrayPrototypeIncludes(
          supportedNamedCurves,
          namedCurve,
        )
      ) {
        const keyData = await op_crypto_generate_key({
          algorithm: "EC",
          namedCurve,
        });
        setKeyData(handle, {
          type: "private",
          data: keyData,
        });
      } else {
        throw new DOMException("Curve not supported", "NotSupportedError");
      }

      // 4-6.
      const algorithm = {
        name: algorithmName,
        namedCurve,
      };

      // 7-11.
      const publicKey = constructKey(
        "public",
        true,
        usageIntersection(usages, ["verify"]),
        algorithm,
        handle,
      );

      // 12-16.
      const privateKey = constructKey(
        "private",
        extractable,
        usageIntersection(usages, ["sign"]),
        algorithm,
        handle,
      );

      // 17-20.
      return { publicKey, privateKey };
    }
    case "ECDH": {
      const namedCurve = normalizedAlgorithm.namedCurve;

      // 1.
      if (
        ArrayPrototypeFind(
          usages,
          (u) => !ArrayPrototypeIncludes(["deriveKey", "deriveBits"], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      // 2-3.
      const handle = {};
      if (
        ArrayPrototypeIncludes(
          supportedNamedCurves,
          namedCurve,
        )
      ) {
        const keyData = await op_crypto_generate_key({
          algorithm: "EC",
          namedCurve,
        });
        setKeyData(handle, {
          type: "private",
          data: keyData,
        });
      } else {
        throw new DOMException("Curve not supported", "NotSupportedError");
      }

      // 4-6.
      const algorithm = {
        name: algorithmName,
        namedCurve,
      };

      // 7-11.
      const publicKey = constructKey(
        "public",
        true,
        usageIntersection(usages, []),
        algorithm,
        handle,
      );

      // 12-16.
      const privateKey = constructKey(
        "private",
        extractable,
        usageIntersection(usages, ["deriveKey", "deriveBits"]),
        algorithm,
        handle,
      );

      // 17-20.
      return { publicKey, privateKey };
    }
    case "AES-CTR":
    case "AES-CBC":
    case "AES-GCM":
    case "AES-OCB": {
      // 1.
      if (
        ArrayPrototypeFind(
          usages,
          (u) =>
            !ArrayPrototypeIncludes([
              "encrypt",
              "decrypt",
              "wrapKey",
              "unwrapKey",
            ], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      return generateKeyAES(normalizedAlgorithm, extractable, usages);
    }
    case "AES-KW": {
      // 1.
      if (
        ArrayPrototypeFind(
          usages,
          (u) => !ArrayPrototypeIncludes(["wrapKey", "unwrapKey"], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      return generateKeyAES(normalizedAlgorithm, extractable, usages);
    }
    case "X448": {
      if (
        ArrayPrototypeFind(
          usages,
          (u) => !ArrayPrototypeIncludes(["deriveKey", "deriveBits"], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }
      const privateKeyData = new Uint8Array(56);
      const publicKeyData = new Uint8Array(56);

      op_crypto_generate_x448_keypair(privateKeyData, publicKeyData);

      const handle = {};
      setKeyData(handle, privateKeyData);

      const publicHandle = {};
      setKeyData(publicHandle, publicKeyData);

      const algorithm = {
        name: algorithmName,
      };

      const publicKey = constructKey(
        "public",
        true,
        usageIntersection(usages, []),
        algorithm,
        publicHandle,
      );

      const privateKey = constructKey(
        "private",
        extractable,
        usageIntersection(usages, ["deriveKey", "deriveBits"]),
        algorithm,
        handle,
      );

      return { publicKey, privateKey };
    }
    case "X25519": {
      if (
        ArrayPrototypeFind(
          usages,
          (u) => !ArrayPrototypeIncludes(["deriveKey", "deriveBits"], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }
      const privateKeyData = new Uint8Array(32);
      const publicKeyData = new Uint8Array(32);
      op_crypto_generate_x25519_keypair(privateKeyData, publicKeyData);

      const handle = {};
      setKeyData(handle, privateKeyData);

      const publicHandle = {};
      setKeyData(publicHandle, publicKeyData);

      const algorithm = {
        name: algorithmName,
      };

      const publicKey = constructKey(
        "public",
        true,
        usageIntersection(usages, []),
        algorithm,
        publicHandle,
      );

      const privateKey = constructKey(
        "private",
        extractable,
        usageIntersection(usages, ["deriveKey", "deriveBits"]),
        algorithm,
        handle,
      );

      return { publicKey, privateKey };
    }
    case "Ed25519": {
      if (
        ArrayPrototypeFind(
          usages,
          (u) => !ArrayPrototypeIncludes(["sign", "verify"], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      const ED25519_SEED_LEN = 32;
      const ED25519_PUBLIC_KEY_LEN = 32;
      const privateKeyData = new Uint8Array(ED25519_SEED_LEN);
      const publicKeyData = new Uint8Array(ED25519_PUBLIC_KEY_LEN);
      if (
        !op_crypto_generate_ed25519_keypair(privateKeyData, publicKeyData)
      ) {
        throw new DOMException("Failed to generate key", "OperationError");
      }

      const handle = {};
      setKeyData(handle, privateKeyData);

      const publicHandle = {};
      setKeyData(publicHandle, publicKeyData);

      const algorithm = {
        name: algorithmName,
      };

      const publicKey = constructKey(
        "public",
        true,
        usageIntersection(usages, ["verify"]),
        algorithm,
        publicHandle,
      );

      const privateKey = constructKey(
        "private",
        extractable,
        usageIntersection(usages, ["sign"]),
        algorithm,
        handle,
      );

      return { publicKey, privateKey };
    }
    case "ML-DSA-44":
    case "ML-DSA-65":
    case "ML-DSA-87": {
      if (
        ArrayPrototypeFind(
          usages,
          (u) => !ArrayPrototypeIncludes(["sign", "verify"], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      const variant = mldsaVariantId(algorithmName);
      const seed = new Uint8Array(32);
      op_crypto_get_random_values(seed);
      const { privateKey: privateKeyBytes, publicKey: publicKeyBytes } =
        op_crypto_mldsa_from_seed(variant, seed);

      const handle = {};
      setKeyData(handle, {
        seed,
        privateKey: privateKeyBytes,
      });

      const publicHandle = {};
      setKeyData(publicHandle, publicKeyBytes);

      const algorithm = {
        name: algorithmName,
      };

      const publicKey = constructKey(
        "public",
        true,
        usageIntersection(usages, ["verify"]),
        algorithm,
        publicHandle,
      );

      const privateKey = constructKey(
        "private",
        extractable,
        usageIntersection(usages, ["sign"]),
        algorithm,
        handle,
      );

      WeakMapPrototypeSet(MLDSA_PUBLIC_FROM_PRIVATE, privateKey, publicKey);

      return { publicKey, privateKey };
    }
    case "ChaCha20-Poly1305": {
      // 1.
      if (
        ArrayPrototypeFind(
          usages,
          (u) =>
            !ArrayPrototypeIncludes([
              "encrypt",
              "decrypt",
              "wrapKey",
              "unwrapKey",
            ], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      // 2. ChaCha20-Poly1305 keys are always 256 bits.
      const keyData = await op_crypto_generate_key({
        algorithm: "AES",
        length: 256,
      });
      const handle = {};
      setKeyData(handle, {
        type: "secret",
        data: keyData,
      });

      const algorithm = {
        name: algorithmName,
      };

      return constructKey(
        "secret",
        extractable,
        usages,
        algorithm,
        handle,
      );
    }
    case "HMAC": {
      // 1.
      if (
        ArrayPrototypeFind(
          usages,
          (u) => !ArrayPrototypeIncludes(["sign", "verify"], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      // 2.
      let length;
      if (normalizedAlgorithm.length === undefined) {
        length = null;
      } else if (normalizedAlgorithm.length !== 0) {
        length = normalizedAlgorithm.length;
      } else {
        throw new DOMException("Invalid length", "OperationError");
      }

      // 3-4.
      const keyData = await op_crypto_generate_key({
        algorithm: "HMAC",
        hash: normalizedAlgorithm.hash.name,
        length,
      });
      const handle = {};
      setKeyData(handle, {
        type: "secret",
        data: keyData,
      });

      // 6-10.
      const algorithm = {
        name: algorithmName,
        hash: {
          name: normalizedAlgorithm.hash.name,
        },
        length: TypedArrayPrototypeGetByteLength(keyData) * 8,
      };

      // 5, 11-13.
      const key = constructKey(
        "secret",
        extractable,
        usages,
        algorithm,
        handle,
      );

      // 14.
      return key;
    }
    case "ML-KEM-512":
    case "ML-KEM-768":
    case "ML-KEM-1024": {
      // ML-KEM (FIPS 203) keys use the encapsulateKey/decapsulateKey usages
      // defined in the WICG Modern Algorithms spec.
      for (let i = 0; i < usages.length; i++) {
        if (
          !ArrayPrototypeIncludes(
            [
              "encapsulateKey",
              "encapsulateBits",
              "decapsulateKey",
              "decapsulateBits",
            ],
            usages[i],
          )
        ) {
          throw new DOMException("Invalid key usage", "SyntaxError");
        }
      }

      // FIPS 203 keys are derived from a 64-byte seed (d || z) so the seed can
      // later be exported (raw-seed / jwk / pkcs8). aws-lc-rs does not expose
      // seed-based generation, so the seed is expanded by the RustCrypto
      // backend; the resulting bytes are FIPS 203 standard.
      const seed = new Uint8Array(64);
      op_crypto_get_random_values(seed);
      const { privateKey: privBytes, publicKey: pubBytes } =
        op_crypto_ml_kem_from_seed(algorithmName, seed);

      const algorithm = { name: algorithmName };

      const privHandle = {};
      setKeyData(privHandle, { seed, privateKey: privBytes });

      const pubHandle = {};
      setKeyData(pubHandle, pubBytes);

      const publicKey = constructKey(
        "public",
        true,
        usageIntersection(usages, ["encapsulateKey", "encapsulateBits"]),
        algorithm,
        pubHandle,
      );
      const privateKey = constructKey(
        "private",
        extractable,
        usageIntersection(usages, ["decapsulateKey", "decapsulateBits"]),
        algorithm,
        privHandle,
      );

      return { publicKey, privateKey };
    }
  }
}

function importKeyX448(
  format,
  keyData,
  extractable,
  keyUsages,
) {
  switch (format) {
    // "raw-public" is an alias of "raw" for existing asymmetric public keys.
    case "raw-public":
    case "raw": {
      // 1.
      if (keyUsages.length > 0) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      if (TypedArrayPrototypeGetByteLength(keyData) !== 56) {
        throw new DOMException("Invalid key data", "DataError");
      }

      const handle = {};
      setKeyData(handle, keyData);

      // 2-3.
      const algorithm = {
        name: "X448",
      };

      // 4-6.
      return constructKey(
        "public",
        extractable,
        [],
        algorithm,
        handle,
      );
    }
    case "spki": {
      // 1.
      if (keyUsages.length > 0) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      const publicKeyData = new Uint8Array(56);
      if (!op_crypto_import_spki_x448(keyData, publicKeyData)) {
        throw new DOMException("Invalid key data", "DataError");
      }

      const handle = {};
      setKeyData(handle, publicKeyData);

      const algorithm = {
        name: "X448",
      };

      return constructKey(
        "public",
        extractable,
        [],
        algorithm,
        handle,
      );
    }
    case "pkcs8": {
      // 1.
      if (
        ArrayPrototypeFind(
          keyUsages,
          (u) => !ArrayPrototypeIncludes(["deriveKey", "deriveBits"], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      const privateKeyData = new Uint8Array(56);
      if (!op_crypto_import_pkcs8_x448(keyData, privateKeyData)) {
        throw new DOMException("Invalid key data", "DataError");
      }

      const handle = {};
      setKeyData(handle, privateKeyData);

      const algorithm = {
        name: "X448",
      };

      return constructKey(
        "private",
        extractable,
        usageIntersection(keyUsages, recognisedUsages),
        algorithm,
        handle,
      );
    }
    case "jwk": {
      // 1.
      const jwk = keyData;

      // 2.
      if (jwk.d !== undefined) {
        if (
          ArrayPrototypeFind(
            keyUsages,
            (u) =>
              !ArrayPrototypeIncludes(
                ["deriveKey", "deriveBits"],
                u,
              ),
          ) !== undefined
        ) {
          throw new DOMException("Invalid key usage", "SyntaxError");
        }
      }

      // 3.
      if (jwk.d === undefined && keyUsages.length > 0) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      // 4.
      if (jwk.kty !== "OKP") {
        throw new DOMException("Invalid key type", "DataError");
      }

      // 5.
      if (jwk.crv !== "X448") {
        throw new DOMException("Invalid curve", "DataError");
      }

      // 6.
      if (keyUsages.length > 0 && jwk.use !== undefined) {
        if (jwk.use !== "enc") {
          throw new DOMException("Invalid key use", "DataError");
        }
      }

      // 7.
      if (jwk.key_ops !== undefined) {
        if (
          ArrayPrototypeFind(
            jwk.key_ops,
            (u) => !ArrayPrototypeIncludes(recognisedUsages, u),
          ) !== undefined
        ) {
          throw new DOMException(
            "'key_ops' property of JsonWebKey is invalid",
            "DataError",
          );
        }

        if (
          !ArrayPrototypeEvery(
            jwk.key_ops,
            (u) => ArrayPrototypeIncludes(keyUsages, u),
          )
        ) {
          throw new DOMException(
            "'key_ops' property of JsonWebKey is invalid",
            "DataError",
          );
        }
      }

      // 8.
      if (jwk.ext !== undefined && jwk.ext === false && extractable) {
        throw new DOMException("Invalid key extractability", "DataError");
      }

      // 9.
      if (jwk.d !== undefined) {
        // https://www.rfc-editor.org/rfc/rfc8037#section-2
        let privateKeyData;
        try {
          privateKeyData = op_crypto_base64url_decode(jwk.d);
        } catch (_) {
          throw new DOMException("Invalid private key data", "DataError");
        }
        if (TypedArrayPrototypeGetByteLength(privateKeyData) !== 56) {
          throw new DOMException("Invalid private key data", "DataError");
        }

        const handle = {};
        setKeyData(handle, privateKeyData);

        const algorithm = {
          name: "X448",
        };

        return constructKey(
          "private",
          extractable,
          usageIntersection(keyUsages, ["deriveKey", "deriveBits"]),
          algorithm,
          handle,
        );
      } else {
        // https://www.rfc-editor.org/rfc/rfc8037#section-2
        let publicKeyData;
        try {
          publicKeyData = op_crypto_base64url_decode(jwk.x);
        } catch (_) {
          throw new DOMException("Invalid public key data", "DataError");
        }
        if (TypedArrayPrototypeGetByteLength(publicKeyData) !== 56) {
          throw new DOMException("Invalid public key data", "DataError");
        }

        const handle = {};
        setKeyData(handle, publicKeyData);

        const algorithm = {
          name: "X448",
        };

        return constructKey(
          "public",
          extractable,
          [],
          algorithm,
          handle,
        );
      }
    }
    default:
      throw new DOMException("Not implemented", "NotSupportedError");
  }
}

function importKeyEd25519(
  format,
  keyData,
  extractable,
  keyUsages,
) {
  switch (format) {
    // "raw-public" is an alias of "raw" for existing asymmetric public keys.
    case "raw-public":
    case "raw": {
      // 1.
      if (
        ArrayPrototypeFind(
          keyUsages,
          (u) => !ArrayPrototypeIncludes(["verify"], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      if (TypedArrayPrototypeGetByteLength(keyData) !== 32) {
        throw new DOMException("Invalid key data", "DataError");
      }

      const handle = {};
      setKeyData(handle, keyData);

      // 2-3.
      const algorithm = {
        name: "Ed25519",
      };

      // 4-6.
      return constructKey(
        "public",
        extractable,
        usageIntersection(keyUsages, recognisedUsages),
        algorithm,
        handle,
      );
    }
    case "spki": {
      // 1.
      if (
        ArrayPrototypeFind(
          keyUsages,
          (u) => !ArrayPrototypeIncludes(["verify"], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      const publicKeyData = new Uint8Array(32);
      if (!op_crypto_import_spki_ed25519(keyData, publicKeyData)) {
        throw new DOMException("Invalid key data", "DataError");
      }

      const handle = {};
      setKeyData(handle, publicKeyData);

      const algorithm = {
        name: "Ed25519",
      };

      return constructKey(
        "public",
        extractable,
        usageIntersection(keyUsages, recognisedUsages),
        algorithm,
        handle,
      );
    }
    case "pkcs8": {
      // 1.
      if (
        ArrayPrototypeFind(
          keyUsages,
          (u) => !ArrayPrototypeIncludes(["sign"], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      const privateKeyData = new Uint8Array(32);
      if (!op_crypto_import_pkcs8_ed25519(keyData, privateKeyData)) {
        throw new DOMException("Invalid key data", "DataError");
      }

      const handle = {};
      setKeyData(handle, privateKeyData);

      const algorithm = {
        name: "Ed25519",
      };

      return constructKey(
        "private",
        extractable,
        usageIntersection(keyUsages, recognisedUsages),
        algorithm,
        handle,
      );
    }
    case "jwk": {
      // 1.
      const jwk = keyData;

      // 2.
      if (jwk.d !== undefined) {
        if (
          ArrayPrototypeFind(
            keyUsages,
            (u) =>
              !ArrayPrototypeIncludes(
                ["sign"],
                u,
              ),
          ) !== undefined
        ) {
          throw new DOMException("Invalid key usage", "SyntaxError");
        }
      } else {
        if (
          ArrayPrototypeFind(
            keyUsages,
            (u) =>
              !ArrayPrototypeIncludes(
                ["verify"],
                u,
              ),
          ) !== undefined
        ) {
          throw new DOMException("Invalid key usage", "SyntaxError");
        }
      }

      // 3.
      if (jwk.kty !== "OKP") {
        throw new DOMException("Invalid key type", "DataError");
      }

      // 4.
      if (jwk.crv !== "Ed25519") {
        throw new DOMException("Invalid curve", "DataError");
      }

      // 5.
      if (
        keyUsages.length > 0 && jwk.use !== undefined && jwk.use !== "sig"
      ) {
        throw new DOMException("Invalid key usage", "DataError");
      }

      // 6.
      if (jwk.key_ops !== undefined) {
        if (
          ArrayPrototypeFind(
            jwk.key_ops,
            (u) => !ArrayPrototypeIncludes(recognisedUsages, u),
          ) !== undefined
        ) {
          throw new DOMException(
            "'key_ops' property of JsonWebKey is invalid",
            "DataError",
          );
        }

        if (
          !ArrayPrototypeEvery(
            jwk.key_ops,
            (u) => ArrayPrototypeIncludes(keyUsages, u),
          )
        ) {
          throw new DOMException(
            "'key_ops' property of JsonWebKey is invalid",
            "DataError",
          );
        }
      }

      // 7.
      if (jwk.ext !== undefined && jwk.ext === false && extractable) {
        throw new DOMException("Invalid key extractability", "DataError");
      }

      // 8.
      if (jwk.d !== undefined) {
        // https://www.rfc-editor.org/rfc/rfc8037#section-2
        let privateKeyData;
        try {
          privateKeyData = op_crypto_base64url_decode(jwk.d);
        } catch (_) {
          throw new DOMException("Invalid private key data", "DataError");
        }
        if (TypedArrayPrototypeGetByteLength(privateKeyData) !== 32) {
          throw new DOMException("Invalid private key data", "DataError");
        }

        const handle = {};
        setKeyData(handle, privateKeyData);

        const algorithm = {
          name: "Ed25519",
        };

        return constructKey(
          "private",
          extractable,
          usageIntersection(keyUsages, recognisedUsages),
          algorithm,
          handle,
        );
      } else {
        // https://www.rfc-editor.org/rfc/rfc8037#section-2
        let publicKeyData;
        try {
          publicKeyData = op_crypto_base64url_decode(jwk.x);
        } catch (_) {
          throw new DOMException("Invalid public key data", "DataError");
        }
        if (TypedArrayPrototypeGetByteLength(publicKeyData) !== 32) {
          throw new DOMException("Invalid public key data", "DataError");
        }

        const handle = {};
        setKeyData(handle, publicKeyData);

        const algorithm = {
          name: "Ed25519",
        };

        return constructKey(
          "public",
          extractable,
          usageIntersection(keyUsages, recognisedUsages),
          algorithm,
          handle,
        );
      }
    }
    default:
      throw new DOMException("Not implemented", "NotSupportedError");
  }
}

function importKeyX25519(
  format,
  keyData,
  extractable,
  keyUsages,
) {
  switch (format) {
    // "raw-public" is an alias of "raw" for existing asymmetric public keys.
    case "raw-public":
    case "raw": {
      // 1.
      if (keyUsages.length > 0) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      if (TypedArrayPrototypeGetByteLength(keyData) !== 32) {
        throw new DOMException("Invalid key data", "DataError");
      }

      const handle = {};
      setKeyData(handle, keyData);

      // 2-3.
      const algorithm = {
        name: "X25519",
      };

      // 4-6.
      return constructKey(
        "public",
        extractable,
        [],
        algorithm,
        handle,
      );
    }
    case "spki": {
      // 1.
      if (keyUsages.length > 0) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      const publicKeyData = new Uint8Array(32);
      if (!op_crypto_import_spki_x25519(keyData, publicKeyData)) {
        throw new DOMException("Invalid key data", "DataError");
      }

      const handle = {};
      setKeyData(handle, publicKeyData);

      const algorithm = {
        name: "X25519",
      };

      return constructKey(
        "public",
        extractable,
        [],
        algorithm,
        handle,
      );
    }
    case "pkcs8": {
      // 1.
      if (
        ArrayPrototypeFind(
          keyUsages,
          (u) => !ArrayPrototypeIncludes(["deriveKey", "deriveBits"], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      const privateKeyData = new Uint8Array(32);
      if (!op_crypto_import_pkcs8_x25519(keyData, privateKeyData)) {
        throw new DOMException("Invalid key data", "DataError");
      }

      const handle = {};
      setKeyData(handle, privateKeyData);

      const algorithm = {
        name: "X25519",
      };

      return constructKey(
        "private",
        extractable,
        usageIntersection(keyUsages, recognisedUsages),
        algorithm,
        handle,
      );
    }
    case "jwk": {
      // 1.
      const jwk = keyData;

      // 2.
      if (jwk.d !== undefined) {
        if (
          ArrayPrototypeFind(
            keyUsages,
            (u) =>
              !ArrayPrototypeIncludes(
                ["deriveKey", "deriveBits"],
                u,
              ),
          ) !== undefined
        ) {
          throw new DOMException("Invalid key usage", "SyntaxError");
        }
      }

      // 3.
      if (jwk.d === undefined && keyUsages.length > 0) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      // 4.
      if (jwk.kty !== "OKP") {
        throw new DOMException("Invalid key type", "DataError");
      }

      // 5.
      if (jwk.crv !== "X25519") {
        throw new DOMException("Invalid curve", "DataError");
      }

      // 6.
      if (keyUsages.length > 0 && jwk.use !== undefined) {
        if (jwk.use !== "enc") {
          throw new DOMException("Invalid key use", "DataError");
        }
      }

      // 7.
      if (jwk.key_ops !== undefined) {
        if (
          ArrayPrototypeFind(
            jwk.key_ops,
            (u) => !ArrayPrototypeIncludes(recognisedUsages, u),
          ) !== undefined
        ) {
          throw new DOMException(
            "'key_ops' property of JsonWebKey is invalid",
            "DataError",
          );
        }

        if (
          !ArrayPrototypeEvery(
            jwk.key_ops,
            (u) => ArrayPrototypeIncludes(keyUsages, u),
          )
        ) {
          throw new DOMException(
            "'key_ops' property of JsonWebKey is invalid",
            "DataError",
          );
        }
      }

      // 8.
      if (jwk.ext !== undefined && jwk.ext === false && extractable) {
        throw new DOMException("Invalid key extractability", "DataError");
      }

      // 9.
      if (jwk.d !== undefined) {
        // https://www.rfc-editor.org/rfc/rfc8037#section-2
        let privateKeyData;
        try {
          privateKeyData = op_crypto_base64url_decode(jwk.d);
        } catch (_) {
          throw new DOMException("Invalid private key data", "DataError");
        }
        if (TypedArrayPrototypeGetByteLength(privateKeyData) !== 32) {
          throw new DOMException("Invalid private key data", "DataError");
        }

        const handle = {};
        setKeyData(handle, privateKeyData);

        const algorithm = {
          name: "X25519",
        };

        return constructKey(
          "private",
          extractable,
          usageIntersection(keyUsages, ["deriveKey", "deriveBits"]),
          algorithm,
          handle,
        );
      } else {
        // https://www.rfc-editor.org/rfc/rfc8037#section-2
        let publicKeyData;
        try {
          publicKeyData = op_crypto_base64url_decode(jwk.x);
        } catch (_) {
          throw new DOMException("Invalid public key data", "DataError");
        }
        if (TypedArrayPrototypeGetByteLength(publicKeyData) !== 32) {
          throw new DOMException("Invalid public key data", "DataError");
        }

        const handle = {};
        setKeyData(handle, publicKeyData);

        const algorithm = {
          name: "X25519",
        };

        return constructKey(
          "public",
          extractable,
          [],
          algorithm,
          handle,
        );
      }
    }
    default:
      throw new DOMException("Not implemented", "NotSupportedError");
  }
}

function exportKeyAES(
  format,
  key,
  innerKey,
) {
  switch (format) {
    // 2.
    // For existing symmetric algorithms "raw" is an alias of "raw-secret".
    // AES-OCB is a newer (tentative) algorithm whose "raw-secret" support is
    // tracked separately, so it is intentionally excluded from the alias here.
    case "raw-secret":
      if (key[_algorithm].name === "AES-OCB") {
        throw new DOMException("Not implemented", "NotSupportedError");
      }
      /* falls through */
    case "raw": {
      // 1.
      const data = innerKey.data;
      // 2.
      return TypedArrayPrototypeGetBuffer(data);
    }
    case "jwk": {
      // 1-2.
      const jwk = {
        kty: "oct",
      };

      // 3.
      const data = op_crypto_export_key({
        format: "jwksecret",
        algorithm: "AES",
      }, innerKey);
      ObjectAssign(jwk, data);

      // 4.
      const algorithm = key[_algorithm];
      switch (algorithm.length) {
        case 128:
          jwk.alg = aesJwkAlg[algorithm.name][128];
          break;
        case 192:
          jwk.alg = aesJwkAlg[algorithm.name][192];
          break;
        case 256:
          jwk.alg = aesJwkAlg[algorithm.name][256];
          break;
        default:
          throw new DOMException(
            `Invalid key length: ${algorithm.length}`,
            "NotSupportedError",
          );
      }

      // 5.
      jwk.key_ops = key.usages;

      // 6.
      jwk.ext = key[_extractable];

      // 7.
      return jwk;
    }
    default:
      throw new DOMException("Not implemented", "NotSupportedError");
  }
}

function exportKeyChaCha20Poly1305(format, key, innerKey) {
  switch (format) {
    // ChaCha20-Poly1305 is a modern symmetric algorithm and therefore only
    // recognizes "raw-secret" (not the "raw" alias).
    case "raw-secret": {
      const data = innerKey.data;
      return TypedArrayPrototypeGetBuffer(data);
    }
    case "jwk": {
      // 1-2.
      const jwk = {
        kty: "oct",
      };

      // 3.
      const data = op_crypto_export_key({
        format: "jwksecret",
        algorithm: "AES",
      }, innerKey);
      jwk.k = data.k;

      // 4.
      jwk.alg = "C20P";

      // 5.
      jwk.key_ops = key.usages;

      // 6.
      jwk.ext = key[_extractable];

      // 7.
      return jwk;
    }
    default:
      throw new DOMException("Not implemented", "NotSupportedError");
  }
}

const ML_KEM_PRIVATE_SIZES = {
  "ML-KEM-512": 1632,
  "ML-KEM-768": 2400,
  "ML-KEM-1024": 3168,
};
const ML_KEM_PUBLIC_SIZES = {
  "ML-KEM-512": 800,
  "ML-KEM-768": 1184,
  "ML-KEM-1024": 1568,
};
const ML_KEM_CIPHERTEXT_SIZES = {
  "ML-KEM-512": 768,
  "ML-KEM-768": 1088,
  "ML-KEM-1024": 1568,
};

const ML_KEM_PRIVATE_USAGES = ["decapsulateKey", "decapsulateBits"];
const ML_KEM_PUBLIC_USAGES = ["encapsulateKey", "encapsulateBits"];

function importKeyMlKem(
  format,
  normalizedAlgorithm,
  keyData,
  extractable,
  keyUsages,
) {
  const algorithmName = normalizedAlgorithm.name;
  const algorithm = { name: algorithmName };

  const makePublicKey = (publicBytes) => {
    const handle = {};
    setKeyData(handle, publicBytes);
    return constructKey(
      "public",
      extractable,
      usageIntersection(keyUsages, ML_KEM_PUBLIC_USAGES),
      algorithm,
      handle,
    );
  };

  // `seed` is the 64-byte FIPS 203 seed, or `null` for keys imported from the
  // expanded form (which carries no recoverable seed). `privateBytes` is the
  // expanded decapsulation key.
  const makePrivateKey = (seed, privateBytes) => {
    const handle = {};
    setKeyData(handle, { seed, privateKey: privateBytes });
    return constructKey(
      "private",
      extractable,
      usageIntersection(keyUsages, ML_KEM_PRIVATE_USAGES),
      algorithm,
      handle,
    );
  };

  switch (format) {
    case "raw-public": {
      // Public encapsulation key.
      const expectedSize = ML_KEM_PUBLIC_SIZES[algorithmName];
      if (TypedArrayPrototypeGetByteLength(keyData) !== expectedSize) {
        throw new DOMException("Invalid key data", "DataError");
      }
      for (let i = 0; i < keyUsages.length; i++) {
        if (!ArrayPrototypeIncludes(ML_KEM_PUBLIC_USAGES, keyUsages[i])) {
          throw new DOMException("Invalid key usage", "SyntaxError");
        }
      }
      if (
        !op_crypto_ml_kem_validate_public_key(algorithmName, keyData)
      ) {
        throw new DOMException("Invalid key data", "DataError");
      }
      return makePublicKey(keyData);
    }
    case "raw-private": {
      // Private decapsulation key in FIPS 203 expanded form. This form carries
      // no seed, so the key cannot later be exported as raw-seed/jwk/pkcs8.
      const expectedSize = ML_KEM_PRIVATE_SIZES[algorithmName];
      if (TypedArrayPrototypeGetByteLength(keyData) !== expectedSize) {
        throw new DOMException("Invalid key data", "DataError");
      }
      for (let i = 0; i < keyUsages.length; i++) {
        if (!ArrayPrototypeIncludes(ML_KEM_PRIVATE_USAGES, keyUsages[i])) {
          throw new DOMException("Invalid key usage", "SyntaxError");
        }
      }
      if (
        !op_crypto_ml_kem_validate_private_key(algorithmName, keyData)
      ) {
        throw new DOMException("Invalid key data", "DataError");
      }
      return makePrivateKey(null, keyData);
    }
    case "raw-seed": {
      // FIPS 203 64-byte seed (d || z).
      for (let i = 0; i < keyUsages.length; i++) {
        if (!ArrayPrototypeIncludes(ML_KEM_PRIVATE_USAGES, keyUsages[i])) {
          throw new DOMException("Invalid key usage", "SyntaxError");
        }
      }
      if (TypedArrayPrototypeGetByteLength(keyData) !== 64) {
        throw new DOMException("Invalid key data", "DataError");
      }
      let res;
      try {
        res = op_crypto_ml_kem_from_seed(algorithmName, keyData);
      } catch (_) {
        throw new DOMException("Invalid key data", "DataError");
      }
      const seedCopy = TypedArrayPrototypeSlice(keyData);
      return makePrivateKey(seedCopy, res.privateKey);
    }
    case "spki": {
      for (let i = 0; i < keyUsages.length; i++) {
        if (!ArrayPrototypeIncludes(ML_KEM_PUBLIC_USAGES, keyUsages[i])) {
          throw new DOMException("Invalid key usage", "SyntaxError");
        }
      }
      let imported;
      try {
        imported = op_crypto_ml_kem_import_spki(keyData);
      } catch (_) {
        throw new DOMException("Invalid key data", "DataError");
      }
      if (imported.variant !== algorithmName) {
        throw new DOMException(
          "Imported key algorithm does not match",
          "DataError",
        );
      }
      return makePublicKey(imported.publicKey);
    }
    case "pkcs8": {
      for (let i = 0; i < keyUsages.length; i++) {
        if (!ArrayPrototypeIncludes(ML_KEM_PRIVATE_USAGES, keyUsages[i])) {
          throw new DOMException("Invalid key usage", "SyntaxError");
        }
      }
      let imported;
      try {
        imported = op_crypto_ml_kem_import_pkcs8(keyData);
      } catch (e) {
        // The expanded-key-only form must be rejected with NotSupportedError;
        // malformed DER and a `both`-form seed/expandedKey mismatch are
        // DataError. (The op's NotSupported class maps to the DOMException
        // name "NotSupported" in Deno; re-throw with the spec name here.)
        if (e?.name === "NotSupported") {
          throw new DOMException(
            "ML-KEM 'expandedKey' PKCS#8 format is not supported; only the " +
              "seed form is supported",
            "NotSupportedError",
          );
        }
        throw new DOMException("Invalid key data", "DataError");
      }
      if (imported.variant !== algorithmName) {
        throw new DOMException(
          "Imported key algorithm does not match",
          "DataError",
        );
      }
      return makePrivateKey(imported.seed, imported.privateKey);
    }
    case "jwk": {
      // 1.
      const jwk = keyData;

      // 2.
      if (jwk.priv !== undefined) {
        if (
          ArrayPrototypeFind(
            keyUsages,
            (u) => !ArrayPrototypeIncludes(ML_KEM_PRIVATE_USAGES, u),
          ) !== undefined
        ) {
          throw new DOMException("Invalid key usage", "SyntaxError");
        }
      } else {
        if (
          ArrayPrototypeFind(
            keyUsages,
            (u) => !ArrayPrototypeIncludes(ML_KEM_PUBLIC_USAGES, u),
          ) !== undefined
        ) {
          throw new DOMException("Invalid key usage", "SyntaxError");
        }
      }

      // 3.
      if (jwk.kty !== "AKP") {
        throw new DOMException("Invalid key type", "DataError");
      }

      // 4.
      if (jwk.alg !== algorithmName) {
        throw new DOMException("Invalid algorithm", "DataError");
      }

      // 5.
      if (
        keyUsages.length > 0 && jwk.use !== undefined && jwk.use !== "enc"
      ) {
        throw new DOMException("Invalid key usage", "DataError");
      }

      // 6.
      if (jwk.key_ops !== undefined) {
        if (
          ArrayPrototypeFind(
            jwk.key_ops,
            (u) => !ArrayPrototypeIncludes(recognisedUsages, u),
          ) !== undefined
        ) {
          throw new DOMException(
            "'key_ops' property of JsonWebKey is invalid",
            "DataError",
          );
        }

        if (
          !ArrayPrototypeEvery(
            jwk.key_ops,
            (u) => ArrayPrototypeIncludes(keyUsages, u),
          )
        ) {
          throw new DOMException(
            "'key_ops' property of JsonWebKey is invalid",
            "DataError",
          );
        }
      }

      // 7.
      if (jwk.ext !== undefined && jwk.ext === false && extractable) {
        throw new DOMException("Invalid key extractability", "DataError");
      }

      // 8.
      if (jwk.priv !== undefined) {
        let seed;
        try {
          seed = op_crypto_base64url_decode(jwk.priv);
        } catch (_) {
          throw new DOMException("Invalid private key data", "DataError");
        }
        if (TypedArrayPrototypeGetByteLength(seed) !== 64) {
          throw new DOMException("Invalid private key data", "DataError");
        }
        let res;
        try {
          res = op_crypto_ml_kem_from_seed(algorithmName, seed);
        } catch (_) {
          throw new DOMException("Invalid private key data", "DataError");
        }

        // The 'pub' field must be present and equal to the public key
        // derived from the seed.
        let pub;
        try {
          pub = op_crypto_base64url_decode(jwk.pub);
        } catch (_) {
          throw new DOMException("Invalid public key data", "DataError");
        }
        if (!bytesEqual(pub, res.publicKey)) {
          throw new DOMException("Invalid public key data", "DataError");
        }

        return makePrivateKey(seed, res.privateKey);
      } else {
        let pub;
        try {
          pub = op_crypto_base64url_decode(jwk.pub);
        } catch (_) {
          throw new DOMException("Invalid public key data", "DataError");
        }
        if (
          TypedArrayPrototypeGetByteLength(pub) !==
            ML_KEM_PUBLIC_SIZES[algorithmName]
        ) {
          throw new DOMException("Invalid public key data", "DataError");
        }
        if (!op_crypto_ml_kem_validate_public_key(algorithmName, pub)) {
          throw new DOMException("Invalid public key data", "DataError");
        }
        return makePublicKey(pub);
      }
    }
    default:
      throw new DOMException(
        "Unsupported key format for ML-KEM",
        "NotSupportedError",
      );
  }
}

function exportKeyMlKem(format, key, innerKey) {
  const algorithmName = key[_algorithm].name;
  const type = key[_type];

  switch (format) {
    case "raw-public": {
      if (type !== "public") {
        throw new DOMException(
          "'raw-public' is only valid for public keys",
          "InvalidAccessError",
        );
      }
      return TypedArrayPrototypeGetBuffer(TypedArrayPrototypeSlice(innerKey));
    }
    case "raw-private": {
      if (type !== "private") {
        throw new DOMException(
          "'raw-private' is only valid for private keys",
          "InvalidAccessError",
        );
      }
      return TypedArrayPrototypeGetBuffer(
        TypedArrayPrototypeSlice(innerKey.privateKey),
      );
    }
    case "raw-seed": {
      if (type !== "private") {
        throw new DOMException(
          "'raw-seed' is only valid for private keys",
          "InvalidAccessError",
        );
      }
      const seed = innerKey?.seed;
      if (seed == null) {
        throw new DOMException(
          "Seed is not available for this key",
          "OperationError",
        );
      }
      return TypedArrayPrototypeGetBuffer(TypedArrayPrototypeSlice(seed));
    }
    case "spki": {
      if (type !== "public") {
        throw new DOMException(
          "'spki' is only valid for public keys",
          "InvalidAccessError",
        );
      }
      const der = op_crypto_ml_kem_export_spki(algorithmName, innerKey);
      return TypedArrayPrototypeGetBuffer(der);
    }
    case "pkcs8": {
      if (type !== "private") {
        throw new DOMException(
          "'pkcs8' is only valid for private keys",
          "InvalidAccessError",
        );
      }
      const seed = innerKey?.seed;
      if (seed == null) {
        throw new DOMException(
          "PKCS#8 export requires the original ML-KEM seed; this key was " +
            "imported without one",
          "OperationError",
        );
      }
      const der = op_crypto_ml_kem_export_pkcs8(algorithmName, seed);
      return TypedArrayPrototypeGetBuffer(der);
    }
    case "jwk": {
      const jwk = {
        kty: "AKP",
        alg: algorithmName,
        "key_ops": key.usages,
        ext: key[_extractable],
      };
      if (type === "private") {
        const seed = innerKey?.seed;
        if (seed == null) {
          throw new DOMException(
            "JWK export requires the original ML-KEM seed; this key was " +
              "imported without one",
            "OperationError",
          );
        }
        const publicKeyBytes = op_crypto_ml_kem_get_public_key(
          algorithmName,
          key[_handle].cppgc,
        );
        jwk.pub = op_crypto_base64url_encode(publicKeyBytes);
        jwk.priv = op_crypto_base64url_encode(seed);
      } else {
        jwk.pub = op_crypto_base64url_encode(innerKey);
      }
      return jwk;
    }
    default:
      throw new DOMException(
        "Unsupported key format for ML-KEM",
        "NotSupportedError",
      );
  }
}

function importKeyChaCha20Poly1305(
  format,
  keyData,
  extractable,
  keyUsages,
) {
  const supportedKeyUsages = ["encrypt", "decrypt", "wrapKey", "unwrapKey"];
  if (
    ArrayPrototypeFind(
      keyUsages,
      (u) => !ArrayPrototypeIncludes(supportedKeyUsages, u),
    ) !== undefined
  ) {
    throw new DOMException("Invalid key usage", "SyntaxError");
  }

  let data;
  switch (format) {
    // ChaCha20-Poly1305 is a modern symmetric algorithm and therefore only
    // recognizes "raw-secret" (not the "raw" alias).
    case "raw-secret": {
      if (TypedArrayPrototypeGetByteLength(keyData) !== 32) {
        throw new DOMException(
          "Invalid key length: ChaCha20-Poly1305 requires 256-bit key",
          "DataError",
        );
      }
      data = keyData;
      break;
    }
    case "jwk": {
      const jwk = keyData;

      // 2.
      if (jwk.kty !== "oct") {
        throw new DOMException(
          "'kty' property of JsonWebKey must be 'oct'",
          "DataError",
        );
      }

      // Section 6.4.1 of RFC7518
      if (jwk.k === undefined) {
        throw new DOMException(
          "'k' property of JsonWebKey must be present",
          "DataError",
        );
      }

      // 4.
      const { rawData } = op_crypto_import_key(
        { algorithm: "AES" },
        { jwkSecret: jwk },
      );
      data = rawData.data;

      // 5.
      if (TypedArrayPrototypeGetByteLength(data) !== 32) {
        throw new DOMException(
          "Invalid key length: ChaCha20-Poly1305 requires 256-bit key",
          "DataError",
        );
      }

      // 6.
      if (jwk.alg !== undefined && jwk.alg !== "C20P") {
        throw new DOMException(`Invalid algorithm: ${jwk.alg}`, "DataError");
      }

      // 7.
      if (
        keyUsages.length > 0 && jwk.use !== undefined && jwk.use !== "enc"
      ) {
        throw new DOMException("Invalid key usage", "DataError");
      }

      // 8.
      // Section 4.3 of RFC7517
      if (jwk.key_ops !== undefined) {
        if (
          ArrayPrototypeFind(
            jwk.key_ops,
            (u) => !ArrayPrototypeIncludes(recognisedUsages, u),
          ) !== undefined
        ) {
          throw new DOMException(
            "'key_ops' property of JsonWebKey is invalid",
            "DataError",
          );
        }

        if (
          !ArrayPrototypeEvery(
            keyUsages,
            (u) => ArrayPrototypeIncludes(jwk.key_ops, u),
          )
        ) {
          throw new DOMException(
            "'key_ops' property of JsonWebKey is invalid",
            "DataError",
          );
        }
      }

      // 9.
      if (jwk.ext === false && extractable === true) {
        throw new DOMException(
          "'ext' property of JsonWebKey must not be false if extractable is true",
          "DataError",
        );
      }
      break;
    }
    default:
      throw new DOMException("Not implemented", "NotSupportedError");
  }

  const handle = {};
  setKeyData(handle, {
    type: "secret",
    data,
  });

  const algorithm = {
    name: "ChaCha20-Poly1305",
  };

  return constructKey(
    "secret",
    extractable,
    usageIntersection(keyUsages, recognisedUsages),
    algorithm,
    handle,
  );
}

function importKeyAES(
  format,
  normalizedAlgorithm,
  keyData,
  extractable,
  keyUsages,
  supportedKeyUsages,
) {
  // 1.
  if (
    ArrayPrototypeFind(
      keyUsages,
      (u) => !ArrayPrototypeIncludes(supportedKeyUsages, u),
    ) !== undefined
  ) {
    throw new DOMException("Invalid key usage", "SyntaxError");
  }

  const algorithmName = normalizedAlgorithm.name;

  // 2.
  let data = keyData;

  switch (format) {
    // For existing symmetric algorithms "raw" is an alias of "raw-secret".
    // AES-OCB is a newer (tentative) algorithm whose "raw-secret" support is
    // tracked separately, so it is intentionally excluded from the alias here.
    case "raw-secret":
      if (algorithmName === "AES-OCB") {
        throw new DOMException("Not implemented", "NotSupportedError");
      }
      /* falls through */
    case "raw": {
      // 2.
      if (
        !ArrayPrototypeIncludes(
          [128, 192, 256],
          TypedArrayPrototypeGetByteLength(keyData) * 8,
        )
      ) {
        throw new DOMException("Invalid key length", "DataError");
      }

      break;
    }
    case "jwk": {
      // 1.
      const jwk = keyData;

      // 2.
      if (jwk.kty !== "oct") {
        throw new DOMException(
          "'kty' property of JsonWebKey must be 'oct'",
          "DataError",
        );
      }

      // Section 6.4.1 of RFC7518
      if (jwk.k === undefined) {
        throw new DOMException(
          "'k' property of JsonWebKey must be present",
          "DataError",
        );
      }

      // 4.
      const { rawData } = op_crypto_import_key(
        { algorithm: "AES" },
        { jwkSecret: jwk },
      );
      data = rawData.data;

      // 5.
      switch (TypedArrayPrototypeGetByteLength(data) * 8) {
        case 128:
          if (
            jwk.alg !== undefined &&
            jwk.alg !== aesJwkAlg[algorithmName][128]
          ) {
            throw new DOMException(
              `Invalid algorithm: ${jwk.alg}`,
              "DataError",
            );
          }
          break;
        case 192:
          if (
            jwk.alg !== undefined &&
            jwk.alg !== aesJwkAlg[algorithmName][192]
          ) {
            throw new DOMException(
              `Invalid algorithm: ${jwk.alg}`,
              "DataError",
            );
          }
          break;
        case 256:
          if (
            jwk.alg !== undefined &&
            jwk.alg !== aesJwkAlg[algorithmName][256]
          ) {
            throw new DOMException(
              `Invalid algorithm: ${jwk.alg}`,
              "DataError",
            );
          }
          break;
        default:
          throw new DOMException(
            "Invalid key length",
            "DataError",
          );
      }

      // 6.
      if (
        keyUsages.length > 0 && jwk.use !== undefined && jwk.use !== "enc"
      ) {
        throw new DOMException("Invalid key usage", "DataError");
      }

      // 7.
      // Section 4.3 of RFC7517
      if (jwk.key_ops !== undefined) {
        if (
          ArrayPrototypeFind(
            jwk.key_ops,
            (u) => !ArrayPrototypeIncludes(recognisedUsages, u),
          ) !== undefined
        ) {
          throw new DOMException(
            "'key_ops' property of JsonWebKey is invalid",
            "DataError",
          );
        }

        if (
          !ArrayPrototypeEvery(
            keyUsages,
            (u) => ArrayPrototypeIncludes(jwk.key_ops, u),
          )
        ) {
          throw new DOMException(
            "'key_ops' property of JsonWebKey is invalid",
            "DataError",
          );
        }
      }

      // 8.
      if (jwk.ext === false && extractable === true) {
        throw new DOMException(
          "'ext' property of JsonWebKey must not be false if extractable is true",
          "DataError",
        );
      }

      break;
    }
    default:
      throw new DOMException("Not implemented", "NotSupportedError");
  }

  const handle = {};
  setKeyData(handle, {
    type: "secret",
    data,
  });

  // 4-7.
  const algorithm = {
    name: algorithmName,
    length: TypedArrayPrototypeGetByteLength(data) * 8,
  };

  const key = constructKey(
    "secret",
    extractable,
    usageIntersection(keyUsages, recognisedUsages),
    algorithm,
    handle,
  );

  // 8.
  return key;
}

function importKeyHMAC(
  format,
  normalizedAlgorithm,
  keyData,
  extractable,
  keyUsages,
) {
  // 2.
  if (
    ArrayPrototypeFind(
      keyUsages,
      (u) => !ArrayPrototypeIncludes(["sign", "verify"], u),
    ) !== undefined
  ) {
    throw new DOMException("Invalid key usage", "SyntaxError");
  }

  // 3.
  let hash;
  let data;

  // 4. https://w3c.github.io/webcrypto/#hmac-operations
  switch (format) {
    // For existing symmetric algorithms "raw" is an alias of "raw-secret".
    case "raw-secret":
    case "raw": {
      data = keyData;
      hash = normalizedAlgorithm.hash;
      break;
    }
    case "jwk": {
      const jwk = keyData;

      // 2.
      if (jwk.kty !== "oct") {
        throw new DOMException(
          "'kty' property of JsonWebKey must be 'oct'",
          "DataError",
        );
      }

      // Section 6.4.1 of RFC7518
      if (jwk.k === undefined) {
        throw new DOMException(
          "'k' property of JsonWebKey must be present",
          "DataError",
        );
      }

      // 4.
      const { rawData } = op_crypto_import_key(
        { algorithm: "HMAC" },
        { jwkSecret: jwk },
      );
      data = rawData.data;

      // 5.
      hash = normalizedAlgorithm.hash;

      // 6.
      switch (hash.name) {
        case "SHA-1": {
          if (jwk.alg !== undefined && jwk.alg !== "HS1") {
            throw new DOMException(
              "'alg' property of JsonWebKey must be 'HS1'",
              "DataError",
            );
          }
          break;
        }
        case "SHA-256": {
          if (jwk.alg !== undefined && jwk.alg !== "HS256") {
            throw new DOMException(
              "'alg' property of JsonWebKey must be 'HS256'",
              "DataError",
            );
          }
          break;
        }
        case "SHA-384": {
          if (jwk.alg !== undefined && jwk.alg !== "HS384") {
            throw new DOMException(
              "'alg' property of JsonWebKey must be 'HS384'",
              "DataError",
            );
          }
          break;
        }
        case "SHA-512": {
          if (jwk.alg !== undefined && jwk.alg !== "HS512") {
            throw new DOMException(
              "'alg' property of JsonWebKey must be 'HS512'",
              "DataError",
            );
          }
          break;
        }
        case "SHA3-256": {
          if (jwk.alg !== undefined && jwk.alg !== "HS3-256") {
            throw new DOMException(
              "'alg' property of JsonWebKey must be 'HS3-256'",
              "DataError",
            );
          }
          break;
        }
        case "SHA3-384": {
          if (jwk.alg !== undefined && jwk.alg !== "HS3-384") {
            throw new DOMException(
              "'alg' property of JsonWebKey must be 'HS3-384'",
              "DataError",
            );
          }
          break;
        }
        case "SHA3-512": {
          if (jwk.alg !== undefined && jwk.alg !== "HS3-512") {
            throw new DOMException(
              "'alg' property of JsonWebKey must be 'HS3-512'",
              "DataError",
            );
          }
          break;
        }
        default:
          throw new TypeError("Unreachable");
      }

      // 7.
      if (
        keyUsages.length > 0 && jwk.use !== undefined && jwk.use !== "sig"
      ) {
        throw new DOMException(
          "'use' property of JsonWebKey must be 'sig'",
          "DataError",
        );
      }

      // 8.
      // Section 4.3 of RFC7517
      if (jwk.key_ops !== undefined) {
        if (
          ArrayPrototypeFind(
            jwk.key_ops,
            (u) => !ArrayPrototypeIncludes(recognisedUsages, u),
          ) !== undefined
        ) {
          throw new DOMException(
            "'key_ops' property of JsonWebKey is invalid",
            "DataError",
          );
        }

        if (
          !ArrayPrototypeEvery(
            keyUsages,
            (u) => ArrayPrototypeIncludes(jwk.key_ops, u),
          )
        ) {
          throw new DOMException(
            "'key_ops' property of JsonWebKey is invalid",
            "DataError",
          );
        }
      }

      // 9.
      if (jwk.ext === false && extractable === true) {
        throw new DOMException(
          "'ext' property of JsonWebKey must not be false if extractable is true",
          "DataError",
        );
      }

      break;
    }
    default:
      throw new DOMException("Not implemented", "NotSupportedError");
  }

  // 5.
  let length = TypedArrayPrototypeGetByteLength(data) * 8;
  // 6.
  if (length === 0) {
    throw new DOMException("Key length is zero", "DataError");
  }
  // 7.
  if (normalizedAlgorithm.length !== undefined) {
    if (
      normalizedAlgorithm.length > length ||
      normalizedAlgorithm.length <= (length - 8)
    ) {
      throw new DOMException(
        "Key length is invalid",
        "DataError",
      );
    }
    length = normalizedAlgorithm.length;
  }

  const handle = {};
  setKeyData(handle, {
    type: "secret",
    data,
  });

  const algorithm = {
    name: "HMAC",
    length,
    hash,
  };

  const key = constructKey(
    "secret",
    extractable,
    usageIntersection(keyUsages, recognisedUsages),
    algorithm,
    handle,
  );

  return key;
}

function importKeyEC(
  format,
  normalizedAlgorithm,
  keyData,
  extractable,
  keyUsages,
) {
  const supportedUsages = SUPPORTED_KEY_USAGES[normalizedAlgorithm.name];

  switch (format) {
    // "raw-public" is an alias of "raw" for existing asymmetric public keys.
    case "raw-public":
    case "raw": {
      // 1.
      if (
        !ArrayPrototypeIncludes(
          supportedNamedCurves,
          normalizedAlgorithm.namedCurve,
        )
      ) {
        throw new DOMException(
          "Invalid namedCurve",
          "DataError",
        );
      }

      // 2.
      if (
        ArrayPrototypeFind(
          keyUsages,
          (u) =>
            !ArrayPrototypeIncludes(
              SUPPORTED_KEY_USAGES[normalizedAlgorithm.name].public,
              u,
            ),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      // 3.
      const { rawData } = op_crypto_import_key({
        algorithm: normalizedAlgorithm.name,
        namedCurve: normalizedAlgorithm.namedCurve,
      }, { raw: keyData });

      const handle = {};
      setKeyData(handle, rawData);

      // 4-5.
      const algorithm = {
        name: normalizedAlgorithm.name,
        namedCurve: normalizedAlgorithm.namedCurve,
      };

      // 6-8.
      const key = constructKey(
        "public",
        extractable,
        usageIntersection(keyUsages, recognisedUsages),
        algorithm,
        handle,
      );

      return key;
    }
    case "pkcs8": {
      // 1.
      if (
        ArrayPrototypeFind(
          keyUsages,
          (u) =>
            !ArrayPrototypeIncludes(
              SUPPORTED_KEY_USAGES[normalizedAlgorithm.name].private,
              u,
            ),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      // 2-9.
      const { rawData } = op_crypto_import_key({
        algorithm: normalizedAlgorithm.name,
        namedCurve: normalizedAlgorithm.namedCurve,
      }, { pkcs8: keyData });

      const handle = {};
      setKeyData(handle, rawData);

      const algorithm = {
        name: normalizedAlgorithm.name,
        namedCurve: normalizedAlgorithm.namedCurve,
      };

      const key = constructKey(
        "private",
        extractable,
        usageIntersection(keyUsages, recognisedUsages),
        algorithm,
        handle,
      );

      return key;
    }
    case "spki": {
      // 1.
      if (normalizedAlgorithm.name == "ECDSA") {
        if (
          ArrayPrototypeFind(
            keyUsages,
            (u) =>
              !ArrayPrototypeIncludes(
                SUPPORTED_KEY_USAGES[normalizedAlgorithm.name].public,
                u,
              ),
          ) !== undefined
        ) {
          throw new DOMException("Invalid key usage", "SyntaxError");
        }
      } else if (keyUsages.length != 0) {
        throw new DOMException("Key usage must be empty", "SyntaxError");
      }

      // 2-12
      const { rawData } = op_crypto_import_key({
        algorithm: normalizedAlgorithm.name,
        namedCurve: normalizedAlgorithm.namedCurve,
      }, { spki: keyData });

      const handle = {};
      setKeyData(handle, rawData);

      const algorithm = {
        name: normalizedAlgorithm.name,
        namedCurve: normalizedAlgorithm.namedCurve,
      };

      // 6-8.
      const key = constructKey(
        "public",
        extractable,
        usageIntersection(keyUsages, recognisedUsages),
        algorithm,
        handle,
      );

      return key;
    }
    case "jwk": {
      const jwk = keyData;

      const keyType = (jwk.d !== undefined) ? "private" : "public";

      // 2.
      if (
        ArrayPrototypeFind(
          keyUsages,
          (u) => !ArrayPrototypeIncludes(supportedUsages[keyType], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      // 3.
      if (jwk.kty !== "EC") {
        throw new DOMException(
          "'kty' property of JsonWebKey must be 'EC'",
          "DataError",
        );
      }

      // 4.
      if (
        keyUsages.length > 0 && jwk.use !== undefined &&
        jwk.use !== supportedUsages.jwkUse
      ) {
        throw new DOMException(
          `'use' property of JsonWebKey must be '${supportedUsages.jwkUse}'`,
          "DataError",
        );
      }

      // 5.
      // Section 4.3 of RFC7517
      if (jwk.key_ops !== undefined) {
        if (
          ArrayPrototypeFind(
            jwk.key_ops,
            (u) => !ArrayPrototypeIncludes(recognisedUsages, u),
          ) !== undefined
        ) {
          throw new DOMException(
            "'key_ops' member of JsonWebKey is invalid",
            "DataError",
          );
        }

        if (
          !ArrayPrototypeEvery(
            keyUsages,
            (u) => ArrayPrototypeIncludes(jwk.key_ops, u),
          )
        ) {
          throw new DOMException(
            "'key_ops' member of JsonWebKey is invalid",
            "DataError",
          );
        }
      }

      // 6.
      if (jwk.ext === false && extractable === true) {
        throw new DOMException(
          "'ext' property of JsonWebKey must not be false if extractable is true",
          "DataError",
        );
      }

      // 9.
      if (jwk.alg !== undefined && normalizedAlgorithm.name == "ECDSA") {
        let algNamedCurve;

        switch (jwk.alg) {
          case "ES256": {
            algNamedCurve = "P-256";
            break;
          }
          case "ES384": {
            algNamedCurve = "P-384";
            break;
          }
          case "ES512": {
            algNamedCurve = "P-521";
            break;
          }
          default:
            throw new DOMException(
              "Curve algorithm not supported",
              "DataError",
            );
        }

        if (algNamedCurve) {
          if (algNamedCurve !== normalizedAlgorithm.namedCurve) {
            throw new DOMException(
              "Mismatched curve algorithm",
              "DataError",
            );
          }
        }
      }

      // Validate that this is a valid public key.
      if (jwk.x === undefined) {
        throw new DOMException(
          "'x' property of JsonWebKey is required for EC keys",
          "DataError",
        );
      }
      if (jwk.y === undefined) {
        throw new DOMException(
          "'y' property of JsonWebKey is required for EC keys",
          "DataError",
        );
      }

      if (jwk.d !== undefined) {
        // it's also a Private key
        const { rawData } = op_crypto_import_key({
          algorithm: normalizedAlgorithm.name,
          namedCurve: normalizedAlgorithm.namedCurve,
        }, { jwkPrivateEc: jwk });

        const handle = {};
        setKeyData(handle, rawData);

        const algorithm = {
          name: normalizedAlgorithm.name,
          namedCurve: normalizedAlgorithm.namedCurve,
        };

        const key = constructKey(
          "private",
          extractable,
          usageIntersection(keyUsages, recognisedUsages),
          algorithm,
          handle,
        );

        return key;
      } else {
        const { rawData } = op_crypto_import_key({
          algorithm: normalizedAlgorithm.name,
          namedCurve: normalizedAlgorithm.namedCurve,
        }, { jwkPublicEc: jwk });

        const handle = {};
        setKeyData(handle, rawData);

        const algorithm = {
          name: normalizedAlgorithm.name,
          namedCurve: normalizedAlgorithm.namedCurve,
        };

        const key = constructKey(
          "public",
          extractable,
          usageIntersection(keyUsages, recognisedUsages),
          algorithm,
          handle,
        );

        return key;
      }
    }
    default:
      throw new DOMException("Not implemented", "NotSupportedError");
  }
}

function importKeyMlDsa(
  format,
  normalizedAlgorithm,
  keyData,
  extractable,
  keyUsages,
) {
  const algorithmName = normalizedAlgorithm.name;
  const variant = mldsaVariantId(algorithmName);

  const makePublicKey = (publicBytes) => {
    const handle = {};
    setKeyData(handle, publicBytes);
    return constructKey(
      "public",
      extractable,
      usageIntersection(keyUsages, ["verify"]),
      { name: algorithmName },
      handle,
    );
  };

  const makePrivateKey = (seed, privateBytes, publicBytes) => {
    const handle = {};
    setKeyData(handle, {
      seed,
      privateKey: privateBytes,
    });
    const privateKey = constructKey(
      "private",
      extractable,
      usageIntersection(keyUsages, ["sign"]),
      { name: algorithmName },
      handle,
    );
    WeakMapPrototypeSet(
      MLDSA_PUBLIC_FROM_PRIVATE,
      privateKey,
      makePublicKey(publicBytes),
    );
    return privateKey;
  };

  switch (format) {
    case "raw-seed": {
      if (
        ArrayPrototypeFind(
          keyUsages,
          (u) => !ArrayPrototypeIncludes(["sign"], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }
      if (TypedArrayPrototypeGetByteLength(keyData) !== 32) {
        throw new DOMException("Invalid key data", "DataError");
      }
      let res;
      try {
        res = op_crypto_mldsa_from_seed(variant, keyData);
      } catch (_) {
        throw new DOMException("Invalid key data", "DataError");
      }
      const seedCopy = TypedArrayPrototypeSlice(keyData);
      return makePrivateKey(seedCopy, res.privateKey, res.publicKey);
    }
    case "raw-private": {
      if (
        ArrayPrototypeFind(
          keyUsages,
          (u) => !ArrayPrototypeIncludes(["sign"], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }
      let res;
      try {
        res = op_crypto_mldsa_from_raw_private(variant, keyData);
      } catch (_) {
        throw new DOMException("Invalid key data", "DataError");
      }
      return makePrivateKey(null, res.privateKey, res.publicKey);
    }
    case "raw-public": {
      if (
        ArrayPrototypeFind(
          keyUsages,
          (u) => !ArrayPrototypeIncludes(["verify"], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }
      const expected = mldsaPublicKeyLen(variant);
      if (TypedArrayPrototypeGetByteLength(keyData) !== expected) {
        throw new DOMException("Invalid key data", "DataError");
      }
      return makePublicKey(TypedArrayPrototypeSlice(keyData));
    }
    case "pkcs8": {
      if (
        ArrayPrototypeFind(
          keyUsages,
          (u) => !ArrayPrototypeIncludes(["sign"], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }
      let res;
      try {
        res = op_crypto_mldsa_from_pkcs8(variant, keyData);
      } catch (_) {
        throw new DOMException("Invalid key data", "DataError");
      }
      return makePrivateKey(
        res.seed !== undefined && res.seed !== null ? res.seed : null,
        res.privateKey,
        res.publicKey,
      );
    }
    case "spki": {
      if (
        ArrayPrototypeFind(
          keyUsages,
          (u) => !ArrayPrototypeIncludes(["verify"], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }
      let pub;
      try {
        pub = op_crypto_mldsa_from_spki(variant, keyData);
      } catch (_) {
        throw new DOMException("Invalid key data", "DataError");
      }
      return makePublicKey(pub);
    }
    case "jwk": {
      // 1.
      const jwk = keyData;

      // 2.
      if (jwk.priv !== undefined) {
        if (
          ArrayPrototypeFind(
            keyUsages,
            (u) => !ArrayPrototypeIncludes(["sign"], u),
          ) !== undefined
        ) {
          throw new DOMException("Invalid key usage", "SyntaxError");
        }
      } else {
        if (
          ArrayPrototypeFind(
            keyUsages,
            (u) => !ArrayPrototypeIncludes(["verify"], u),
          ) !== undefined
        ) {
          throw new DOMException("Invalid key usage", "SyntaxError");
        }
      }

      // 3.
      if (jwk.kty !== "AKP") {
        throw new DOMException("Invalid key type", "DataError");
      }

      // 4.
      if (jwk.alg !== algorithmName) {
        throw new DOMException("Invalid algorithm", "DataError");
      }

      // 5.
      if (
        keyUsages.length > 0 && jwk.use !== undefined && jwk.use !== "sig"
      ) {
        throw new DOMException("Invalid key usage", "DataError");
      }

      // 6.
      if (jwk.key_ops !== undefined) {
        if (
          ArrayPrototypeFind(
            jwk.key_ops,
            (u) => !ArrayPrototypeIncludes(recognisedUsages, u),
          ) !== undefined
        ) {
          throw new DOMException(
            "'key_ops' property of JsonWebKey is invalid",
            "DataError",
          );
        }

        if (
          !ArrayPrototypeEvery(
            jwk.key_ops,
            (u) => ArrayPrototypeIncludes(keyUsages, u),
          )
        ) {
          throw new DOMException(
            "'key_ops' property of JsonWebKey is invalid",
            "DataError",
          );
        }
      }

      // 7.
      if (jwk.ext !== undefined && jwk.ext === false && extractable) {
        throw new DOMException("Invalid key extractability", "DataError");
      }

      // 8.
      if (jwk.priv !== undefined) {
        let seed;
        try {
          seed = op_crypto_base64url_decode(jwk.priv);
        } catch (_) {
          throw new DOMException("Invalid private key data", "DataError");
        }
        if (TypedArrayPrototypeGetByteLength(seed) !== 32) {
          throw new DOMException("Invalid private key data", "DataError");
        }
        let res;
        try {
          res = op_crypto_mldsa_from_seed(variant, seed);
        } catch (_) {
          throw new DOMException("Invalid private key data", "DataError");
        }

        // The 'pub' field must be present and equal to the public key
        // derived from the seed.
        let pub;
        try {
          pub = op_crypto_base64url_decode(jwk.pub);
        } catch (_) {
          throw new DOMException("Invalid public key data", "DataError");
        }
        if (!bytesEqual(pub, res.publicKey)) {
          throw new DOMException("Invalid public key data", "DataError");
        }

        return makePrivateKey(seed, res.privateKey, res.publicKey);
      } else {
        let pub;
        try {
          pub = op_crypto_base64url_decode(jwk.pub);
        } catch (_) {
          throw new DOMException("Invalid public key data", "DataError");
        }
        if (
          TypedArrayPrototypeGetByteLength(pub) !== mldsaPublicKeyLen(variant)
        ) {
          throw new DOMException("Invalid public key data", "DataError");
        }
        return makePublicKey(pub);
      }
    }
    default:
      throw new DOMException("Not implemented", "NotSupportedError");
  }
}

function exportKeyMlDsa(format, key, innerKey) {
  const algorithmName = key[_algorithm].name;
  const variant = mldsaVariantId(algorithmName);

  switch (format) {
    case "raw-seed": {
      if (key[_type] !== "private") {
        throw new DOMException(
          "Key is not a private key",
          "InvalidAccessError",
        );
      }
      const seed = innerKey?.seed;
      if (seed == null) {
        throw new DOMException(
          "Seed is not available for this key",
          "OperationError",
        );
      }
      return TypedArrayPrototypeGetBuffer(TypedArrayPrototypeSlice(seed));
    }
    case "raw-private": {
      if (key[_type] !== "private") {
        throw new DOMException(
          "Key is not a private key",
          "InvalidAccessError",
        );
      }
      return TypedArrayPrototypeGetBuffer(
        TypedArrayPrototypeSlice(innerKey.privateKey),
      );
    }
    case "raw-public": {
      if (key[_type] !== "public") {
        throw new DOMException(
          "Key is not a public key",
          "InvalidAccessError",
        );
      }
      return TypedArrayPrototypeGetBuffer(TypedArrayPrototypeSlice(innerKey));
    }
    case "pkcs8": {
      if (key[_type] !== "private") {
        throw new DOMException(
          "Key is not a private key",
          "InvalidAccessError",
        );
      }
      const seed = innerKey?.seed;
      if (seed == null) {
        throw new DOMException(
          "PKCS#8 export requires the original ML-DSA seed; this key was " +
            "imported without one",
          "OperationError",
        );
      }
      const der = op_crypto_mldsa_export_pkcs8(variant, seed);
      return TypedArrayPrototypeGetBuffer(der);
    }
    case "spki": {
      if (key[_type] !== "public") {
        throw new DOMException(
          "Key is not a public key",
          "InvalidAccessError",
        );
      }
      const der = op_crypto_mldsa_export_spki(variant, innerKey);
      return TypedArrayPrototypeGetBuffer(der);
    }
    case "jwk": {
      const jwk = {
        kty: "AKP",
        alg: algorithmName,
        "key_ops": key.usages,
        ext: key[_extractable],
      };
      if (key[_type] === "private") {
        const seed = innerKey?.seed;
        if (seed == null) {
          throw new DOMException(
            "JWK export requires the original ML-DSA seed; this key was " +
              "imported without one",
            "OperationError",
          );
        }
        const publicKey = WeakMapPrototypeGet(MLDSA_PUBLIC_FROM_PRIVATE, key);
        jwk.pub = op_crypto_base64url_encode(getKeyData(publicKey[_handle]));
        jwk.priv = op_crypto_base64url_encode(seed);
      } else {
        jwk.pub = op_crypto_base64url_encode(innerKey);
      }
      return jwk;
    }
    default:
      throw new DOMException("Not implemented", "NotSupportedError");
  }
}

// deno-lint-ignore require-await
async function importKeyInner(
  format,
  normalizedAlgorithm,
  keyData,
  extractable,
  keyUsages,
) {
  const algorithmName = normalizedAlgorithm.name;

  switch (algorithmName) {
    case "HMAC": {
      return importKeyHMAC(
        format,
        normalizedAlgorithm,
        keyData,
        extractable,
        keyUsages,
      );
    }
    case "ECDH":
    case "ECDSA": {
      return importKeyEC(
        format,
        normalizedAlgorithm,
        keyData,
        extractable,
        keyUsages,
      );
    }
    case "RSASSA-PKCS1-v1_5":
    case "RSA-PSS":
    case "RSA-OAEP": {
      return importKeyRSA(
        format,
        normalizedAlgorithm,
        keyData,
        extractable,
        keyUsages,
      );
    }
    case "HKDF": {
      return importKeyHKDF(format, keyData, extractable, keyUsages);
    }
    case "PBKDF2": {
      return importKeyPBKDF2(format, keyData, extractable, keyUsages);
    }
    case "AES-CTR":
    case "AES-CBC":
    case "AES-GCM":
    case "AES-OCB": {
      return importKeyAES(
        format,
        normalizedAlgorithm,
        keyData,
        extractable,
        keyUsages,
        ["encrypt", "decrypt", "wrapKey", "unwrapKey"],
      );
    }
    case "AES-KW": {
      return importKeyAES(
        format,
        normalizedAlgorithm,
        keyData,
        extractable,
        keyUsages,
        ["wrapKey", "unwrapKey"],
      );
    }
    case "ChaCha20-Poly1305": {
      return importKeyChaCha20Poly1305(
        format,
        keyData,
        extractable,
        keyUsages,
      );
    }
    case "X448": {
      return importKeyX448(
        format,
        keyData,
        extractable,
        keyUsages,
      );
    }
    case "X25519": {
      return importKeyX25519(
        format,
        keyData,
        extractable,
        keyUsages,
      );
    }
    case "Ed25519": {
      return importKeyEd25519(
        format,
        keyData,
        extractable,
        keyUsages,
      );
    }
    case "ML-KEM-512":
    case "ML-KEM-768":
    case "ML-KEM-1024": {
      return importKeyMlKem(
        format,
        normalizedAlgorithm,
        keyData,
        extractable,
        keyUsages,
      );
    }
    case "ML-DSA-44":
    case "ML-DSA-65":
    case "ML-DSA-87": {
      return importKeyMlDsa(
        format,
        normalizedAlgorithm,
        keyData,
        extractable,
        keyUsages,
      );
    }
    default:
      throw new DOMException("Not implemented", "NotSupportedError");
  }
}

const SUPPORTED_KEY_USAGES = {
  "RSASSA-PKCS1-v1_5": {
    public: ["verify"],
    private: ["sign"],
    jwkUse: "sig",
  },
  "RSA-PSS": {
    public: ["verify"],
    private: ["sign"],
    jwkUse: "sig",
  },
  "RSA-OAEP": {
    public: ["encrypt", "wrapKey"],
    private: ["decrypt", "unwrapKey"],
    jwkUse: "enc",
  },
  "ECDSA": {
    public: ["verify"],
    private: ["sign"],
    jwkUse: "sig",
  },
  "ECDH": {
    public: [],
    private: ["deriveKey", "deriveBits"],
    jwkUse: "enc",
  },
};

function importKeyRSA(
  format,
  normalizedAlgorithm,
  keyData,
  extractable,
  keyUsages,
) {
  switch (format) {
    case "pkcs8": {
      // 1.
      if (
        ArrayPrototypeFind(
          keyUsages,
          (u) =>
            !ArrayPrototypeIncludes(
              SUPPORTED_KEY_USAGES[normalizedAlgorithm.name].private,
              u,
            ),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      // 2-9.
      const { modulusLength, publicExponent, rawData } = op_crypto_import_key(
        {
          algorithm: normalizedAlgorithm.name,
          // Needed to perform step 7 without normalization.
          hash: normalizedAlgorithm.hash.name,
        },
        { pkcs8: keyData },
      );

      const handle = {};
      setKeyData(handle, rawData);

      const algorithm = {
        name: normalizedAlgorithm.name,
        modulusLength,
        publicExponent,
        hash: normalizedAlgorithm.hash,
      };

      const key = constructKey(
        "private",
        extractable,
        usageIntersection(keyUsages, recognisedUsages),
        algorithm,
        handle,
      );

      return key;
    }
    case "spki": {
      // 1.
      if (
        ArrayPrototypeFind(
          keyUsages,
          (u) =>
            !ArrayPrototypeIncludes(
              SUPPORTED_KEY_USAGES[normalizedAlgorithm.name].public,
              u,
            ),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      // 2-9.
      const { modulusLength, publicExponent, rawData } = op_crypto_import_key(
        {
          algorithm: normalizedAlgorithm.name,
          // Needed to perform step 7 without normalization.
          hash: normalizedAlgorithm.hash.name,
        },
        { spki: keyData },
      );

      const handle = {};
      setKeyData(handle, rawData);

      const algorithm = {
        name: normalizedAlgorithm.name,
        modulusLength,
        publicExponent,
        hash: normalizedAlgorithm.hash,
      };

      const key = constructKey(
        "public",
        extractable,
        usageIntersection(keyUsages, recognisedUsages),
        algorithm,
        handle,
      );

      return key;
    }
    case "jwk": {
      // 1.
      const jwk = keyData;

      // 2.
      if (jwk.d !== undefined) {
        if (
          ArrayPrototypeFind(
            keyUsages,
            (u) =>
              !ArrayPrototypeIncludes(
                SUPPORTED_KEY_USAGES[normalizedAlgorithm.name].private,
                u,
              ),
          ) !== undefined
        ) {
          throw new DOMException("Invalid key usage", "SyntaxError");
        }
      } else if (
        ArrayPrototypeFind(
          keyUsages,
          (u) =>
            !ArrayPrototypeIncludes(
              SUPPORTED_KEY_USAGES[normalizedAlgorithm.name].public,
              u,
            ),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }

      // 3.
      if (StringPrototypeToUpperCase(jwk.kty) !== "RSA") {
        throw new DOMException(
          "'kty' property of JsonWebKey must be 'RSA'",
          "DataError",
        );
      }

      // 4.
      if (
        keyUsages.length > 0 && jwk.use !== undefined &&
        StringPrototypeToLowerCase(jwk.use) !==
          SUPPORTED_KEY_USAGES[normalizedAlgorithm.name].jwkUse
      ) {
        throw new DOMException(
          `'use' property of JsonWebKey must be '${
            SUPPORTED_KEY_USAGES[normalizedAlgorithm.name].jwkUse
          }'`,
          "DataError",
        );
      }

      // 5.
      if (jwk.key_ops !== undefined) {
        if (
          ArrayPrototypeFind(
            jwk.key_ops,
            (u) => !ArrayPrototypeIncludes(recognisedUsages, u),
          ) !== undefined
        ) {
          throw new DOMException(
            "'key_ops' property of JsonWebKey is invalid",
            "DataError",
          );
        }

        if (
          !ArrayPrototypeEvery(
            keyUsages,
            (u) => ArrayPrototypeIncludes(jwk.key_ops, u),
          )
        ) {
          throw new DOMException(
            "'key_ops' property of JsonWebKey is invalid",
            "DataError",
          );
        }
      }

      if (jwk.ext === false && extractable === true) {
        throw new DOMException(
          "'ext' property of JsonWebKey must not be false if extractable is true",
          "DataError",
        );
      }

      // 7.
      let hash;

      // 8.
      if (normalizedAlgorithm.name === "RSASSA-PKCS1-v1_5") {
        switch (jwk.alg) {
          case undefined:
            hash = undefined;
            break;
          case "RS1":
            hash = "SHA-1";
            break;
          case "RS256":
            hash = "SHA-256";
            break;
          case "RS384":
            hash = "SHA-384";
            break;
          case "RS512":
            hash = "SHA-512";
            break;
          case "RS3-256":
            hash = "SHA3-256";
            break;
          case "RS3-384":
            hash = "SHA3-384";
            break;
          case "RS3-512":
            hash = "SHA3-512";
            break;
          default:
            throw new DOMException(
              `'alg' property of JsonWebKey must be one of 'RS1', 'RS256', 'RS384', 'RS512', 'RS3-256', 'RS3-384', 'RS3-512': received ${jwk.alg}`,
              "DataError",
            );
        }
      } else if (normalizedAlgorithm.name === "RSA-PSS") {
        switch (jwk.alg) {
          case undefined:
            hash = undefined;
            break;
          case "PS1":
            hash = "SHA-1";
            break;
          case "PS256":
            hash = "SHA-256";
            break;
          case "PS384":
            hash = "SHA-384";
            break;
          case "PS512":
            hash = "SHA-512";
            break;
          case "PS3-256":
            hash = "SHA3-256";
            break;
          case "PS3-384":
            hash = "SHA3-384";
            break;
          case "PS3-512":
            hash = "SHA3-512";
            break;
          default:
            throw new DOMException(
              `'alg' property of JsonWebKey must be one of 'PS1', 'PS256', 'PS384', 'PS512', 'PS3-256', 'PS3-384', 'PS3-512': received ${jwk.alg}`,
              "DataError",
            );
        }
      } else {
        switch (jwk.alg) {
          case undefined:
            hash = undefined;
            break;
          case "RSA-OAEP":
            hash = "SHA-1";
            break;
          case "RSA-OAEP-256":
            hash = "SHA-256";
            break;
          case "RSA-OAEP-384":
            hash = "SHA-384";
            break;
          case "RSA-OAEP-512":
            hash = "SHA-512";
            break;
          case "RSA-OAEP3-256":
            hash = "SHA3-256";
            break;
          case "RSA-OAEP3-384":
            hash = "SHA3-384";
            break;
          case "RSA-OAEP3-512":
            hash = "SHA3-512";
            break;
          default:
            throw new DOMException(
              `'alg' property of JsonWebKey must be one of 'RSA-OAEP', 'RSA-OAEP-256', 'RSA-OAEP-384', 'RSA-OAEP-512', 'RSA-OAEP3-256', 'RSA-OAEP3-384', or 'RSA-OAEP3-512': received ${jwk.alg}`,
              "DataError",
            );
        }
      }

      // 9.
      if (hash !== undefined) {
        // 9.1.
        const normalizedHash = normalizeAlgorithm(hash, "digest");

        // 9.2.
        if (normalizedHash.name !== normalizedAlgorithm.hash.name) {
          throw new DOMException(
            `'alg' property of JsonWebKey must be '${normalizedAlgorithm.name}': received ${jwk.alg}`,
            "DataError",
          );
        }
      }

      // 10.
      if (jwk.d !== undefined) {
        // Private key
        const optimizationsPresent = jwk.p !== undefined ||
          jwk.q !== undefined || jwk.dp !== undefined ||
          jwk.dq !== undefined || jwk.qi !== undefined;
        if (optimizationsPresent) {
          if (jwk.q === undefined) {
            throw new DOMException(
              "'q' property of JsonWebKey is required for private keys",
              "DataError",
            );
          }
          if (jwk.dp === undefined) {
            throw new DOMException(
              "'dp' property of JsonWebKey is required for private keys",
              "DataError",
            );
          }
          if (jwk.dq === undefined) {
            throw new DOMException(
              "'dq' property of JsonWebKey is required for private keys",
              "DataError",
            );
          }
          if (jwk.qi === undefined) {
            throw new DOMException(
              "'qi' property of JsonWebKey is required for private keys",
              "DataError",
            );
          }
          if (jwk.oth !== undefined) {
            throw new DOMException(
              "'oth' property of JsonWebKey is not supported",
              "NotSupportedError",
            );
          }
        } else {
          throw new DOMException(
            "Only optimized private keys are supported",
            "NotSupportedError",
          );
        }

        const { modulusLength, publicExponent, rawData } = op_crypto_import_key(
          {
            algorithm: normalizedAlgorithm.name,
            hash: normalizedAlgorithm.hash.name,
          },
          { jwkPrivateRsa: jwk },
        );

        const handle = {};
        setKeyData(handle, rawData);

        const algorithm = {
          name: normalizedAlgorithm.name,
          modulusLength,
          publicExponent,
          hash: normalizedAlgorithm.hash,
        };

        const key = constructKey(
          "private",
          extractable,
          usageIntersection(keyUsages, recognisedUsages),
          algorithm,
          handle,
        );

        return key;
      } else {
        // Validate that this is a valid public key.
        if (jwk.n === undefined) {
          throw new DOMException(
            "'n' property of JsonWebKey is required for public keys",
            "DataError",
          );
        }
        if (jwk.e === undefined) {
          throw new DOMException(
            "'e' property of JsonWebKey is required for public keys",
            "DataError",
          );
        }

        const { modulusLength, publicExponent, rawData } = op_crypto_import_key(
          {
            algorithm: normalizedAlgorithm.name,
            hash: normalizedAlgorithm.hash.name,
          },
          { jwkPublicRsa: jwk },
        );

        const handle = {};
        setKeyData(handle, rawData);

        const algorithm = {
          name: normalizedAlgorithm.name,
          modulusLength,
          publicExponent,
          hash: normalizedAlgorithm.hash,
        };

        const key = constructKey(
          "public",
          extractable,
          usageIntersection(keyUsages, recognisedUsages),
          algorithm,
          handle,
        );

        return key;
      }
    }
    default:
      throw new DOMException("Not implemented", "NotSupportedError");
  }
}

function importKeyHKDF(
  format,
  keyData,
  extractable,
  keyUsages,
) {
  // For existing symmetric algorithms "raw" is an alias of "raw-secret".
  if (format !== "raw" && format !== "raw-secret") {
    throw new DOMException("Format not supported", "NotSupportedError");
  }

  // 1.
  if (
    ArrayPrototypeFind(
      keyUsages,
      (u) => !ArrayPrototypeIncludes(["deriveKey", "deriveBits"], u),
    ) !== undefined
  ) {
    throw new DOMException("Invalid key usage", "SyntaxError");
  }

  // 2.
  if (extractable !== false) {
    throw new DOMException(
      "Key must not be extractable",
      "SyntaxError",
    );
  }

  // 3.
  const handle = {};
  setKeyData(handle, {
    type: "secret",
    data: keyData,
  });

  // 4-8.
  const algorithm = {
    name: "HKDF",
  };
  const key = constructKey(
    "secret",
    false,
    usageIntersection(keyUsages, recognisedUsages),
    algorithm,
    handle,
  );

  // 9.
  return key;
}

function importKeyPBKDF2(
  format,
  keyData,
  extractable,
  keyUsages,
) {
  // 1.
  // For existing symmetric algorithms "raw" is an alias of "raw-secret".
  if (format !== "raw" && format !== "raw-secret") {
    throw new DOMException("Format not supported", "NotSupportedError");
  }

  // 2.
  if (
    ArrayPrototypeFind(
      keyUsages,
      (u) => !ArrayPrototypeIncludes(["deriveKey", "deriveBits"], u),
    ) !== undefined
  ) {
    throw new DOMException("Invalid key usage", "SyntaxError");
  }

  // 3.
  if (extractable !== false) {
    throw new DOMException(
      "Key must not be extractable",
      "SyntaxError",
    );
  }

  // 4.
  const handle = {};
  setKeyData(handle, {
    type: "secret",
    data: keyData,
  });

  // 5-9.
  const algorithm = {
    name: "PBKDF2",
  };
  const key = constructKey(
    "secret",
    false,
    usageIntersection(keyUsages, recognisedUsages),
    algorithm,
    handle,
  );

  // 10.
  return key;
}

function exportKeyHMAC(format, key, innerKey) {
  // 1.
  if (innerKey == null) {
    throw new DOMException("Key is not available", "OperationError");
  }

  switch (format) {
    // 3.
    // For existing symmetric algorithms "raw" is an alias of "raw-secret".
    case "raw-secret":
    case "raw": {
      const bits = innerKey.data;
      // TODO(petamoriken): Uint8Array does not have push method
      // for (let _i = 7 & (8 - bits.length % 8); _i > 0; _i--) {
      //   bits.push(0);
      // }
      // 4-5.
      return TypedArrayPrototypeGetBuffer(bits);
    }
    case "jwk": {
      // 1-2.
      const jwk = {
        kty: "oct",
      };

      // 3.
      const data = op_crypto_export_key({
        format: "jwksecret",
        algorithm: key[_algorithm].name,
      }, innerKey);
      jwk.k = data.k;

      // 4.
      const algorithm = key[_algorithm];
      // 5.
      const hash = algorithm.hash;
      // 6.
      switch (hash.name) {
        case "SHA-1":
          jwk.alg = "HS1";
          break;
        case "SHA-256":
          jwk.alg = "HS256";
          break;
        case "SHA-384":
          jwk.alg = "HS384";
          break;
        case "SHA-512":
          jwk.alg = "HS512";
          break;
        case "SHA3-256":
          jwk.alg = "HS3-256";
          break;
        case "SHA3-384":
          jwk.alg = "HS3-384";
          break;
        case "SHA3-512":
          jwk.alg = "HS3-512";
          break;
        default:
          throw new DOMException(
            "Hash algorithm not supported",
            "NotSupportedError",
          );
      }
      // 7.
      jwk.key_ops = key.usages;
      // 8.
      jwk.ext = key[_extractable];
      // 9.
      return jwk;
    }
    default:
      throw new DOMException("Not implemented", "NotSupportedError");
  }
}

function exportKeyRSA(format, key, innerKey) {
  switch (format) {
    case "pkcs8": {
      // 1.
      if (key[_type] !== "private") {
        throw new DOMException(
          "Key is not a private key",
          "InvalidAccessError",
        );
      }

      // 2.
      const data = op_crypto_export_key({
        algorithm: key[_algorithm].name,
        format: "pkcs8",
      }, innerKey);

      // 3.
      return TypedArrayPrototypeGetBuffer(data);
    }
    case "spki": {
      // 1.
      if (key[_type] !== "public") {
        throw new DOMException(
          "Key is not a public key",
          "InvalidAccessError",
        );
      }

      // 2.
      const data = op_crypto_export_key({
        algorithm: key[_algorithm].name,
        format: "spki",
      }, innerKey);

      // 3.
      return TypedArrayPrototypeGetBuffer(data);
    }
    case "jwk": {
      // 1-2.
      const jwk = {
        kty: "RSA",
      };

      // 3.
      const hash = key[_algorithm].hash.name;

      // 4.
      if (key[_algorithm].name === "RSASSA-PKCS1-v1_5") {
        switch (hash) {
          case "SHA-1":
            jwk.alg = "RS1";
            break;
          case "SHA-256":
            jwk.alg = "RS256";
            break;
          case "SHA-384":
            jwk.alg = "RS384";
            break;
          case "SHA-512":
            jwk.alg = "RS512";
            break;
          case "SHA3-256":
            jwk.alg = "RS3-256";
            break;
          case "SHA3-384":
            jwk.alg = "RS3-384";
            break;
          case "SHA3-512":
            jwk.alg = "RS3-512";
            break;
          default:
            throw new DOMException(
              "Hash algorithm not supported",
              "NotSupportedError",
            );
        }
      } else if (key[_algorithm].name === "RSA-PSS") {
        switch (hash) {
          case "SHA-1":
            jwk.alg = "PS1";
            break;
          case "SHA-256":
            jwk.alg = "PS256";
            break;
          case "SHA-384":
            jwk.alg = "PS384";
            break;
          case "SHA-512":
            jwk.alg = "PS512";
            break;
          case "SHA3-256":
            jwk.alg = "PS3-256";
            break;
          case "SHA3-384":
            jwk.alg = "PS3-384";
            break;
          case "SHA3-512":
            jwk.alg = "PS3-512";
            break;
          default:
            throw new DOMException(
              "Hash algorithm not supported",
              "NotSupportedError",
            );
        }
      } else {
        switch (hash) {
          case "SHA-1":
            jwk.alg = "RSA-OAEP";
            break;
          case "SHA-256":
            jwk.alg = "RSA-OAEP-256";
            break;
          case "SHA-384":
            jwk.alg = "RSA-OAEP-384";
            break;
          case "SHA-512":
            jwk.alg = "RSA-OAEP-512";
            break;
          case "SHA3-256":
            jwk.alg = "RSA-OAEP3-256";
            break;
          case "SHA3-384":
            jwk.alg = "RSA-OAEP3-384";
            break;
          case "SHA3-512":
            jwk.alg = "RSA-OAEP3-512";
            break;
          default:
            throw new DOMException(
              "Hash algorithm not supported",
              "NotSupportedError",
            );
        }
      }

      // 5-6.
      const data = op_crypto_export_key({
        format: key[_type] === "private" ? "jwkprivate" : "jwkpublic",
        algorithm: key[_algorithm].name,
      }, innerKey);
      ObjectAssign(jwk, data);

      // 7.
      jwk.key_ops = key.usages;

      // 8.
      jwk.ext = key[_extractable];

      return jwk;
    }
    default:
      throw new DOMException("Not implemented", "NotSupportedError");
  }
}

function exportKeyEd25519(format, key, innerKey) {
  switch (format) {
    // "raw-public" is an alias of "raw" for existing asymmetric public keys.
    case "raw-public":
    case "raw": {
      // 1.
      if (key[_type] !== "public") {
        throw new DOMException(
          "Key is not a public key",
          "InvalidAccessError",
        );
      }

      // 2-3.
      return TypedArrayPrototypeGetBuffer(innerKey);
    }
    case "spki": {
      // 1.
      if (key[_type] !== "public") {
        throw new DOMException(
          "Key is not a public key",
          "InvalidAccessError",
        );
      }

      const spkiDer = op_crypto_export_spki_ed25519(innerKey);
      return TypedArrayPrototypeGetBuffer(spkiDer);
    }
    case "pkcs8": {
      // 1.
      if (key[_type] !== "private") {
        throw new DOMException(
          "Key is not a public key",
          "InvalidAccessError",
        );
      }

      const pkcs8Der = op_crypto_export_pkcs8_ed25519(
        new Uint8Array([0x04, 0x22, ...new SafeArrayIterator(innerKey)]),
      );
      pkcs8Der[15] = 0x20;
      return TypedArrayPrototypeGetBuffer(pkcs8Der);
    }
    case "jwk": {
      const x = key[_type] === "private"
        ? op_crypto_jwk_x_ed25519(innerKey)
        : op_crypto_base64url_encode(innerKey);
      const jwk = {
        kty: "OKP",
        crv: "Ed25519",
        x,
        "key_ops": key.usages,
        ext: key[_extractable],
      };
      if (key[_type] === "private") {
        jwk.d = op_crypto_base64url_encode(innerKey);
      }
      return jwk;
    }
    default:
      throw new DOMException("Not implemented", "NotSupportedError");
  }
}

function exportKeyX448(format, key, innerKey) {
  switch (format) {
    // "raw-public" is an alias of "raw" for existing asymmetric public keys.
    case "raw-public":
    case "raw": {
      // 1.
      if (key[_type] !== "public") {
        throw new DOMException(
          "Key is not a public key",
          "InvalidAccessError",
        );
      }

      // 2-3.
      return TypedArrayPrototypeGetBuffer(innerKey);
    }
    case "spki": {
      // 1.
      if (key[_type] !== "public") {
        throw new DOMException(
          "Key is not a public key",
          "InvalidAccessError",
        );
      }

      const spkiDer = op_crypto_export_spki_x448(innerKey);
      return TypedArrayPrototypeGetBuffer(spkiDer);
    }
    case "pkcs8": {
      // 1.
      if (key[_type] !== "private") {
        throw new DOMException(
          "Key is not a private key",
          "InvalidAccessError",
        );
      }

      const pkcs8Der = op_crypto_export_pkcs8_x448(
        new Uint8Array([0x04, 0x3a, ...new SafeArrayIterator(innerKey)]),
      );
      pkcs8Der[15] = 0x38;
      return TypedArrayPrototypeGetBuffer(pkcs8Der);
    }
    case "jwk": {
      if (key[_type] === "private") {
        throw new DOMException("Not implemented", "NotSupportedError");
      }
      const x = op_crypto_base64url_encode(innerKey);
      const jwk = {
        kty: "OKP",
        crv: "X448",
        x,
        "key_ops": key.usages,
        ext: key[_extractable],
      };
      return jwk;
    }
    default:
      throw new DOMException("Not implemented", "NotSupportedError");
  }
}

function exportKeyX25519(format, key, innerKey) {
  switch (format) {
    // "raw-public" is an alias of "raw" for existing asymmetric public keys.
    case "raw-public":
    case "raw": {
      // 1.
      if (key[_type] !== "public") {
        throw new DOMException(
          "Key is not a public key",
          "InvalidAccessError",
        );
      }

      // 2-3.
      return TypedArrayPrototypeGetBuffer(innerKey);
    }
    case "spki": {
      // 1.
      if (key[_type] !== "public") {
        throw new DOMException(
          "Key is not a public key",
          "InvalidAccessError",
        );
      }

      const spkiDer = op_crypto_export_spki_x25519(innerKey);
      return TypedArrayPrototypeGetBuffer(spkiDer);
    }
    case "pkcs8": {
      // 1.
      if (key[_type] !== "private") {
        throw new DOMException(
          "Key is not a public key",
          "InvalidAccessError",
        );
      }

      const pkcs8Der = op_crypto_export_pkcs8_x25519(
        new Uint8Array([0x04, 0x22, ...new SafeArrayIterator(innerKey)]),
      );
      pkcs8Der[15] = 0x20;
      return TypedArrayPrototypeGetBuffer(pkcs8Der);
    }
    case "jwk": {
      const jwk = {
        kty: "OKP",
        crv: "X25519",
        "key_ops": key.usages,
        ext: key[_extractable],
      };
      if (key[_type] === "private") {
        jwk.x = op_crypto_x25519_public_key(innerKey);
        jwk.d = op_crypto_base64url_encode(innerKey);
      } else {
        jwk.x = op_crypto_base64url_encode(innerKey);
      }
      return jwk;
    }
    default:
      throw new DOMException("Not implemented", "NotSupportedError");
  }
}

function exportKeyEC(format, key, innerKey) {
  switch (format) {
    // "raw-public" is an alias of "raw" for existing asymmetric public keys.
    case "raw-public":
    case "raw": {
      // 1.
      if (key[_type] !== "public") {
        throw new DOMException(
          "Key is not a public key",
          "InvalidAccessError",
        );
      }

      // 2.
      const data = op_crypto_export_key({
        algorithm: key[_algorithm].name,
        namedCurve: key[_algorithm].namedCurve,
        format: "raw",
      }, innerKey);

      return TypedArrayPrototypeGetBuffer(data);
    }
    case "pkcs8": {
      // 1.
      if (key[_type] !== "private") {
        throw new DOMException(
          "Key is not a private key",
          "InvalidAccessError",
        );
      }

      // 2.
      const data = op_crypto_export_key({
        algorithm: key[_algorithm].name,
        namedCurve: key[_algorithm].namedCurve,
        format: "pkcs8",
      }, innerKey);

      return TypedArrayPrototypeGetBuffer(data);
    }
    case "spki": {
      // 1.
      if (key[_type] !== "public") {
        throw new DOMException(
          "Key is not a public key",
          "InvalidAccessError",
        );
      }

      // 2.
      const data = op_crypto_export_key({
        algorithm: key[_algorithm].name,
        namedCurve: key[_algorithm].namedCurve,
        format: "spki",
      }, innerKey);

      return TypedArrayPrototypeGetBuffer(data);
    }
    case "jwk": {
      if (key[_algorithm].name == "ECDSA") {
        // 1-2.
        const jwk = {
          kty: "EC",
        };

        // 3.1
        jwk.crv = key[_algorithm].namedCurve;

        // Missing from spec
        let algNamedCurve;

        switch (key[_algorithm].namedCurve) {
          case "P-256": {
            algNamedCurve = "ES256";
            break;
          }
          case "P-384": {
            algNamedCurve = "ES384";
            break;
          }
          case "P-521": {
            algNamedCurve = "ES512";
            break;
          }
          default:
            throw new DOMException(
              "Curve algorithm not supported",
              "DataError",
            );
        }

        jwk.alg = algNamedCurve;

        // 3.2 - 3.4.
        const data = op_crypto_export_key({
          format: key[_type] === "private" ? "jwkprivate" : "jwkpublic",
          algorithm: key[_algorithm].name,
          namedCurve: key[_algorithm].namedCurve,
        }, innerKey);
        ObjectAssign(jwk, data);

        // 4.
        jwk.key_ops = key.usages;

        // 5.
        jwk.ext = key[_extractable];

        return jwk;
      } else { // ECDH
        // 1-2.
        const jwk = {
          kty: "EC",
        };

        // missing step from spec
        jwk.alg = "ECDH";

        // 3.1
        jwk.crv = key[_algorithm].namedCurve;

        // 3.2 - 3.4
        const data = op_crypto_export_key({
          format: key[_type] === "private" ? "jwkprivate" : "jwkpublic",
          algorithm: key[_algorithm].name,
          namedCurve: key[_algorithm].namedCurve,
        }, innerKey);
        ObjectAssign(jwk, data);

        // 4.
        jwk.key_ops = key.usages;

        // 5.
        jwk.ext = key[_extractable];

        return jwk;
      }
    }
    default:
      throw new DOMException("Not implemented", "NotSupportedError");
  }
}

async function generateKeyAES(normalizedAlgorithm, extractable, usages) {
  const algorithmName = normalizedAlgorithm.name;

  // 2.
  if (!ArrayPrototypeIncludes([128, 192, 256], normalizedAlgorithm.length)) {
    throw new DOMException(
      `Invalid key length: ${normalizedAlgorithm.length}`,
      "OperationError",
    );
  }

  // 3.
  const keyData = await op_crypto_generate_key({
    algorithm: "AES",
    length: normalizedAlgorithm.length,
  });
  const handle = {};
  setKeyData(handle, {
    type: "secret",
    data: keyData,
  });

  // 6-8.
  const algorithm = {
    name: algorithmName,
    length: normalizedAlgorithm.length,
  };

  // 9-11.
  const key = constructKey(
    "secret",
    extractable,
    usages,
    algorithm,
    handle,
  );

  // 12.
  return key;
}

async function deriveBits(normalizedAlgorithm, baseKey, length) {
  switch (normalizedAlgorithm.name) {
    case "PBKDF2": {
      // 1.
      if (length == null || length == 0 || length % 8 !== 0) {
        throw new DOMException("Invalid length", "OperationError");
      }

      if (normalizedAlgorithm.iterations == 0) {
        throw new DOMException(
          "iterations must not be zero",
          "OperationError",
        );
      }

      const handle = baseKey[_handle];

      normalizedAlgorithm.salt = copyBuffer(normalizedAlgorithm.salt);

      const buf = await op_crypto_derive_bits(handle.cppgc, null, {
        algorithm: "PBKDF2",
        hash: normalizedAlgorithm.hash.name,
        iterations: normalizedAlgorithm.iterations,
        length,
      }, normalizedAlgorithm.salt);

      return TypedArrayPrototypeGetBuffer(buf);
    }
    case "ECDH": {
      // 1.
      if (baseKey[_type] !== "private") {
        throw new DOMException("Invalid key type", "InvalidAccessError");
      }
      // 2.
      const publicKey = normalizedAlgorithm.public;
      // 3.
      if (publicKey[_type] !== "public") {
        throw new DOMException("Invalid key type", "InvalidAccessError");
      }
      // 4.
      if (publicKey[_algorithm].name !== baseKey[_algorithm].name) {
        throw new DOMException(
          "Algorithm mismatch",
          "InvalidAccessError",
        );
      }
      // 5.
      if (
        publicKey[_algorithm].namedCurve !== baseKey[_algorithm].namedCurve
      ) {
        throw new DOMException(
          "'namedCurve' mismatch",
          "InvalidAccessError",
        );
      }
      // 6.
      if (
        ArrayPrototypeIncludes(
          supportedNamedCurves,
          publicKey[_algorithm].namedCurve,
        )
      ) {
        const baseKeyhandle = baseKey[_handle];
        const publicKeyhandle = publicKey[_handle];

        const buf = await op_crypto_derive_bits(
          baseKeyhandle.cppgc,
          publicKeyhandle.cppgc,
          {
            algorithm: "ECDH",
            namedCurve: publicKey[_algorithm].namedCurve,
            length: length ?? 0,
          },
        );

        // 8.
        if (length === null) {
          return TypedArrayPrototypeGetBuffer(buf);
        } else if (TypedArrayPrototypeGetByteLength(buf) * 8 < length) {
          throw new DOMException("Invalid length", "OperationError");
        } else {
          return ArrayBufferPrototypeSlice(
            TypedArrayPrototypeGetBuffer(buf),
            0,
            MathCeil(length / 8),
          );
        }
      } else {
        throw new DOMException("Not implemented", "NotSupportedError");
      }
    }
    case "HKDF": {
      // 1.
      if (length === null || length === 0 || length % 8 !== 0) {
        throw new DOMException("Invalid length", "OperationError");
      }

      const handle = baseKey[_handle];

      normalizedAlgorithm.salt = copyBuffer(normalizedAlgorithm.salt);

      normalizedAlgorithm.info = copyBuffer(normalizedAlgorithm.info);

      const buf = await op_crypto_derive_bits(handle.cppgc, null, {
        algorithm: "HKDF",
        hash: normalizedAlgorithm.hash.name,
        info: normalizedAlgorithm.info,
        length,
      }, normalizedAlgorithm.salt);

      return TypedArrayPrototypeGetBuffer(buf);
    }
    case "X448": {
      // 1.
      if (baseKey[_type] !== "private") {
        throw new DOMException("Invalid key type", "InvalidAccessError");
      }
      // 2.
      const publicKey = normalizedAlgorithm.public;
      // 3.
      if (publicKey[_type] !== "public") {
        throw new DOMException("Invalid key type", "InvalidAccessError");
      }
      // 4.
      if (publicKey[_algorithm].name !== baseKey[_algorithm].name) {
        throw new DOMException(
          "Algorithm mismatch",
          "InvalidAccessError",
        );
      }

      // 5.
      const kHandle = baseKey[_handle];
      const uHandle = publicKey[_handle];

      const secret = new Uint8Array(56);
      const isIdentity = op_crypto_derive_bits_x448(
        kHandle.cppgc,
        uHandle.cppgc,
        secret,
      );

      // 6.
      if (isIdentity) {
        throw new DOMException("Invalid key", "OperationError");
      }

      // 7.
      if (length === null) {
        return TypedArrayPrototypeGetBuffer(secret);
      } else if (
        TypedArrayPrototypeGetByteLength(secret) * 8 < length
      ) {
        throw new DOMException("Invalid length", "OperationError");
      } else {
        return ArrayBufferPrototypeSlice(
          TypedArrayPrototypeGetBuffer(secret),
          0,
          MathCeil(length / 8),
        );
      }
    }
    case "X25519": {
      // 1.
      if (baseKey[_type] !== "private") {
        throw new DOMException("Invalid key type", "InvalidAccessError");
      }
      // 2.
      const publicKey = normalizedAlgorithm.public;
      // 3.
      if (publicKey[_type] !== "public") {
        throw new DOMException("Invalid key type", "InvalidAccessError");
      }
      // 4.
      if (publicKey[_algorithm].name !== baseKey[_algorithm].name) {
        throw new DOMException(
          "Algorithm mismatch",
          "InvalidAccessError",
        );
      }

      // 5.
      const kHandle = baseKey[_handle];
      const uHandle = publicKey[_handle];

      const secret = new Uint8Array(32);
      const isIdentity = op_crypto_derive_bits_x25519(
        kHandle.cppgc,
        uHandle.cppgc,
        secret,
      );

      // 6.
      if (isIdentity) {
        throw new DOMException("Invalid key", "OperationError");
      }

      // 7.
      if (length === null) {
        return TypedArrayPrototypeGetBuffer(secret);
      } else if (
        TypedArrayPrototypeGetByteLength(secret) * 8 < length
      ) {
        throw new DOMException("Invalid length", "OperationError");
      } else {
        return ArrayBufferPrototypeSlice(
          TypedArrayPrototypeGetBuffer(secret),
          0,
          MathCeil(length / 8),
        );
      }
    }
    default:
      throw new DOMException("Not implemented", "NotSupportedError");
  }
}

async function encrypt(normalizedAlgorithm, key, data) {
  const handle = key[_handle];

  switch (normalizedAlgorithm.name) {
    case "RSA-OAEP": {
      // 1.
      if (key[_type] !== "public") {
        throw new DOMException(
          "Key type not supported",
          "InvalidAccessError",
        );
      }

      // 2.
      if (normalizedAlgorithm.label) {
        normalizedAlgorithm.label = copyBuffer(normalizedAlgorithm.label);
      } else {
        normalizedAlgorithm.label = new Uint8Array();
      }

      // 3-5.
      const hashAlgorithm = key[_algorithm].hash.name;
      const cipherText = await op_crypto_encrypt(handle.cppgc, {
        algorithm: "RSA-OAEP",
        hash: hashAlgorithm,
        label: normalizedAlgorithm.label,
      }, data);

      // 6.
      return TypedArrayPrototypeGetBuffer(cipherText);
    }
    case "AES-CBC": {
      normalizedAlgorithm.iv = copyBuffer(normalizedAlgorithm.iv);

      // 1.
      if (TypedArrayPrototypeGetByteLength(normalizedAlgorithm.iv) !== 16) {
        throw new DOMException(
          "Initialization vector must be 16 bytes",
          "OperationError",
        );
      }

      // 2.
      const cipherText = await op_crypto_encrypt(handle.cppgc, {
        algorithm: "AES-CBC",
        length: key[_algorithm].length,
        iv: normalizedAlgorithm.iv,
      }, data);

      // 4.
      return TypedArrayPrototypeGetBuffer(cipherText);
    }
    case "AES-CTR": {
      normalizedAlgorithm.counter = copyBuffer(normalizedAlgorithm.counter);

      // 1.
      if (
        TypedArrayPrototypeGetByteLength(normalizedAlgorithm.counter) !== 16
      ) {
        throw new DOMException(
          "Counter vector must be 16 bytes",
          "OperationError",
        );
      }

      // 2.
      if (
        normalizedAlgorithm.length == 0 || normalizedAlgorithm.length > 128
      ) {
        throw new DOMException(
          "Counter length must not be 0 or greater than 128",
          "OperationError",
        );
      }

      // 3.
      const cipherText = await op_crypto_encrypt(handle.cppgc, {
        algorithm: "AES-CTR",
        keyLength: key[_algorithm].length,
        counter: normalizedAlgorithm.counter,
        ctrLength: normalizedAlgorithm.length,
      }, data);

      // 4.
      return TypedArrayPrototypeGetBuffer(cipherText);
    }
    case "AES-GCM": {
      normalizedAlgorithm.iv = copyBuffer(normalizedAlgorithm.iv);

      // 1.
      if (TypedArrayPrototypeGetByteLength(data) > (2 ** 39) - 256) {
        throw new DOMException(
          "Plaintext too large",
          "OperationError",
        );
      }

      // 2.
      // We only support 96-bit and 128-bit nonce.
      if (
        ArrayPrototypeIncludes(
          [12, 16],
          TypedArrayPrototypeGetByteLength(normalizedAlgorithm.iv),
        ) === undefined
      ) {
        throw new DOMException(
          "Initialization vector length not supported",
          "NotSupportedError",
        );
      }

      // 3.
      // NOTE: over the size of Number.MAX_SAFE_INTEGER is not available in V8
      // if (normalizedAlgorithm.additionalData !== undefined) {
      //   if (normalizedAlgorithm.additionalData.byteLength > (2 ** 64) - 1) {
      //     throw new DOMException(
      //       "Additional data too large",
      //       "OperationError",
      //     );
      //   }
      // }

      // 4.
      if (normalizedAlgorithm.tagLength == undefined) {
        normalizedAlgorithm.tagLength = 128;
      } else if (
        !ArrayPrototypeIncludes(
          [32, 64, 96, 104, 112, 120, 128],
          normalizedAlgorithm.tagLength,
        )
      ) {
        throw new DOMException(
          `Invalid tag length: ${normalizedAlgorithm.tagLength}`,
          "OperationError",
        );
      }
      // 5.
      if (normalizedAlgorithm.additionalData) {
        normalizedAlgorithm.additionalData = copyBuffer(
          normalizedAlgorithm.additionalData,
        );
      }
      // 6-7.
      const cipherText = await op_crypto_encrypt(handle.cppgc, {
        algorithm: "AES-GCM",
        length: key[_algorithm].length,
        iv: normalizedAlgorithm.iv,
        additionalData: normalizedAlgorithm.additionalData || null,
        tagLength: normalizedAlgorithm.tagLength,
      }, data);

      // 8.
      return TypedArrayPrototypeGetBuffer(cipherText);
    }
    case "AES-OCB": {
      normalizedAlgorithm.iv = copyBuffer(normalizedAlgorithm.iv);

      // 1.
      if (TypedArrayPrototypeGetByteLength(data) > (2 ** 39) - 256) {
        throw new DOMException(
          "Plaintext too large",
          "OperationError",
        );
      }

      // 2.
      // OCB supports nonce sizes from 1 to 15 bytes (recommended: 12 bytes)
      const ivLen = TypedArrayPrototypeGetByteLength(normalizedAlgorithm.iv);
      if (ivLen < 1 || ivLen > 15) {
        throw new DOMException(
          "Invalid nonce length for AES-OCB (must be 1-15 bytes)",
          "OperationError",
        );
      }

      // 3.
      if (normalizedAlgorithm.tagLength === undefined) {
        normalizedAlgorithm.tagLength = 128;
      } else if (
        !ArrayPrototypeIncludes(
          [32, 64, 96, 104, 112, 120, 128],
          normalizedAlgorithm.tagLength,
        )
      ) {
        throw new DOMException(
          `Invalid tag length: ${normalizedAlgorithm.tagLength}`,
          "OperationError",
        );
      }
      // 4.
      if (normalizedAlgorithm.additionalData) {
        normalizedAlgorithm.additionalData = copyBuffer(
          normalizedAlgorithm.additionalData,
        );
      }
      // 5-6.
      const cipherText = await op_crypto_encrypt(handle.cppgc, {
        algorithm: "AES-OCB",
        length: key[_algorithm].length,
        iv: normalizedAlgorithm.iv,
        additionalData: normalizedAlgorithm.additionalData || null,
        tagLength: normalizedAlgorithm.tagLength,
      }, data);

      // 7.
      return TypedArrayPrototypeGetBuffer(cipherText);
    }
    case "ChaCha20-Poly1305": {
      if (normalizedAlgorithm.iv === undefined) {
        throw new TypeError("iv is required");
      }
      normalizedAlgorithm.iv = copyBuffer(normalizedAlgorithm.iv);
      if (TypedArrayPrototypeGetByteLength(normalizedAlgorithm.iv) !== 12) {
        throw new DOMException(
          "ChaCha20-Poly1305 iv must be 12 bytes",
          "OperationError",
        );
      }
      // RFC 8439 plaintext size cap.
      if (TypedArrayPrototypeGetByteLength(data) > ((2 ** 32) - 1) * 64) {
        throw new DOMException("Plaintext too large", "OperationError");
      }
      if (normalizedAlgorithm.additionalData !== undefined) {
        normalizedAlgorithm.additionalData = copyBuffer(
          normalizedAlgorithm.additionalData,
        );
      }

      const cipherText = await op_crypto_encrypt(handle.cppgc, {
        algorithm: "ChaCha20-Poly1305",
        nonce: normalizedAlgorithm.iv,
        additionalData: normalizedAlgorithm.additionalData || null,
      }, data);

      return TypedArrayPrototypeGetBuffer(cipherText);
    }
    default:
      throw new DOMException("Not implemented", "NotSupportedError");
  }
}

webidl.configureInterface(SubtleCrypto);
const subtle = webidl.createBranded(SubtleCrypto);

class Crypto {
  constructor() {
    webidl.illegalConstructor();
  }

  getRandomValues(typedArray) {
    webidl.assertBranded(this, CryptoPrototype);
    const prefix = "Failed to execute 'getRandomValues' on 'Crypto'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    // Fast path for Uint8Array
    const tag = TypedArrayPrototypeGetSymbolToStringTag(typedArray);
    if (tag === "Uint8Array") {
      op_crypto_get_random_values(typedArray);
      return typedArray;
    }
    switch (tag) {
      case "Int8Array":
      case "Uint8ClampedArray":
      case "Int16Array":
      case "Uint16Array":
      case "Int32Array":
      case "Uint32Array":
      case "BigInt64Array":
      case "BigUint64Array":
        break;
      default:
        throw new DOMException(
          "The provided value is not an integer-type TypedArray",
          "TypeMismatchError",
        );
    }
    const ui8 = new Uint8Array(
      TypedArrayPrototypeGetBuffer(typedArray),
      TypedArrayPrototypeGetByteOffset(typedArray),
      TypedArrayPrototypeGetByteLength(typedArray),
    );
    op_crypto_get_random_values(ui8);
    return typedArray;
  }

  randomUUID() {
    webidl.assertBranded(this, CryptoPrototype);
    return op_crypto_random_uuid();
  }

  get subtle() {
    webidl.assertBranded(this, CryptoPrototype);
    return subtle;
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(CryptoPrototype, this),
        keys: ["subtle"],
      }),
      inspectOptions,
    );
  }
}

webidl.configureInterface(Crypto);
const CryptoPrototype = Crypto.prototype;

const crypto = webidl.createBranded(Crypto);

webidl.converters.AlgorithmIdentifier = (V, prefix, context, opts) => {
  // Union for (object or DOMString)
  if (webidl.type(V) == "Object") {
    return webidl.converters.object(V, prefix, context, opts);
  }
  return webidl.converters.DOMString(V, prefix, context, opts);
};

webidl.converters["BufferSource or JsonWebKey"] = (
  V,
  prefix,
  context,
  opts,
) => {
  // Union for (BufferSource or JsonWebKey)
  if (ArrayBufferIsView(V) || isArrayBuffer(V)) {
    return webidl.converters.BufferSource(V, prefix, context, opts);
  }
  return webidl.converters.JsonWebKey(V, prefix, context, opts);
};

webidl.converters.KeyType = webidl.createEnumConverter("KeyType", [
  "public",
  "private",
  "secret",
]);

webidl.converters.KeyFormat = webidl.createEnumConverter("KeyFormat", [
  "raw",
  "pkcs8",
  "spki",
  "jwk",
  // WICG modern algorithms: unified symmetric secret key format. For the
  // existing symmetric algorithms `raw` is treated as an alias of `raw-secret`,
  // while new algorithms (e.g. ChaCha20-Poly1305) only recognize `raw-secret`.
  "raw-secret",
  // WICG modern algorithms (ML-KEM, ML-DSA): split raw key formats.
  "raw-public",
  "raw-private",
  "raw-seed",
]);

webidl.converters.KeyUsage = webidl.createEnumConverter("KeyUsage", [
  "encrypt",
  "decrypt",
  "sign",
  "verify",
  "deriveKey",
  "deriveBits",
  "wrapKey",
  "unwrapKey",
  // WICG modern algorithms (ML-KEM): KEM-specific usages.
  "encapsulateKey",
  "encapsulateBits",
  "decapsulateKey",
  "decapsulateBits",
]);

webidl.converters["sequence<KeyUsage>"] = webidl.createSequenceConverter(
  webidl.converters.KeyUsage,
);

webidl.converters.HashAlgorithmIdentifier =
  webidl.converters.AlgorithmIdentifier;

/** @type {webidl.Dictionary} */
const dictAlgorithm = [{
  key: "name",
  converter: webidl.converters.DOMString,
  required: true,
}];

webidl.converters.Algorithm = webidl
  .createDictionaryConverter("Algorithm", dictAlgorithm);

webidl.converters.BigInteger = webidl.converters.Uint8Array;

/** @type {webidl.Dictionary} */
const dictRsaKeyGenParams = [
  ...new SafeArrayIterator(dictAlgorithm),
  {
    key: "modulusLength",
    converter: (V, prefix, context, opts) =>
      webidl.converters["unsigned long"](V, prefix, context, {
        ...opts,
        enforceRange: true,
      }),
    required: true,
  },
  {
    key: "publicExponent",
    converter: webidl.converters.BigInteger,
    required: true,
  },
];

webidl.converters.RsaKeyGenParams = webidl
  .createDictionaryConverter("RsaKeyGenParams", dictRsaKeyGenParams);

const dictRsaHashedKeyGenParams = [
  ...new SafeArrayIterator(dictRsaKeyGenParams),
  {
    key: "hash",
    converter: webidl.converters.HashAlgorithmIdentifier,
    required: true,
  },
];

webidl.converters.RsaHashedKeyGenParams = webidl.createDictionaryConverter(
  "RsaHashedKeyGenParams",
  dictRsaHashedKeyGenParams,
);

const dictRsaHashedImportParams = [
  ...new SafeArrayIterator(dictAlgorithm),
  {
    key: "hash",
    converter: webidl.converters.HashAlgorithmIdentifier,
    required: true,
  },
];

webidl.converters.RsaHashedImportParams = webidl.createDictionaryConverter(
  "RsaHashedImportParams",
  dictRsaHashedImportParams,
);

webidl.converters.NamedCurve = webidl.converters.DOMString;

const dictEcKeyImportParams = [
  ...new SafeArrayIterator(dictAlgorithm),
  {
    key: "namedCurve",
    converter: webidl.converters.NamedCurve,
    required: true,
  },
];

webidl.converters.EcKeyImportParams = webidl.createDictionaryConverter(
  "EcKeyImportParams",
  dictEcKeyImportParams,
);

const dictEcKeyGenParams = [
  ...new SafeArrayIterator(dictAlgorithm),
  {
    key: "namedCurve",
    converter: webidl.converters.NamedCurve,
    required: true,
  },
];

webidl.converters.EcKeyGenParams = webidl
  .createDictionaryConverter("EcKeyGenParams", dictEcKeyGenParams);

const dictAesKeyGenParams = [
  ...new SafeArrayIterator(dictAlgorithm),
  {
    key: "length",
    converter: (V, prefix, context, opts) =>
      webidl.converters["unsigned short"](V, prefix, context, {
        ...opts,
        enforceRange: true,
      }),
    required: true,
  },
];

webidl.converters.AesKeyGenParams = webidl
  .createDictionaryConverter("AesKeyGenParams", dictAesKeyGenParams);

const dictHmacKeyGenParams = [
  ...new SafeArrayIterator(dictAlgorithm),
  {
    key: "hash",
    converter: webidl.converters.HashAlgorithmIdentifier,
    required: true,
  },
  {
    key: "length",
    converter: (V, prefix, context, opts) =>
      webidl.converters["unsigned long"](V, prefix, context, {
        ...opts,
        enforceRange: true,
      }),
  },
];

webidl.converters.HmacKeyGenParams = webidl
  .createDictionaryConverter("HmacKeyGenParams", dictHmacKeyGenParams);

const dictRsaPssParams = [
  ...new SafeArrayIterator(dictAlgorithm),
  {
    key: "saltLength",
    converter: (V, prefix, context, opts) =>
      webidl.converters["unsigned long"](V, prefix, context, {
        ...opts,
        enforceRange: true,
      }),
    required: true,
  },
];

webidl.converters.RsaPssParams = webidl
  .createDictionaryConverter("RsaPssParams", dictRsaPssParams);

const dictRsaOaepParams = [
  ...new SafeArrayIterator(dictAlgorithm),
  {
    key: "label",
    converter: webidl.converters["BufferSource"],
  },
];

webidl.converters.RsaOaepParams = webidl
  .createDictionaryConverter("RsaOaepParams", dictRsaOaepParams);

const dictEcdsaParams = [
  ...new SafeArrayIterator(dictAlgorithm),
  {
    key: "hash",
    converter: webidl.converters.HashAlgorithmIdentifier,
    required: true,
  },
];

webidl.converters["EcdsaParams"] = webidl
  .createDictionaryConverter("EcdsaParams", dictEcdsaParams);

const dictHmacImportParams = [
  ...new SafeArrayIterator(dictAlgorithm),
  {
    key: "hash",
    converter: webidl.converters.HashAlgorithmIdentifier,
    required: true,
  },
  {
    key: "length",
    converter: (V, prefix, context, opts) =>
      webidl.converters["unsigned long"](V, prefix, context, {
        ...opts,
        enforceRange: true,
      }),
  },
];

webidl.converters.HmacImportParams = webidl
  .createDictionaryConverter("HmacImportParams", dictHmacImportParams);

const dictRsaOtherPrimesInfo = [
  {
    key: "r",
    converter: webidl.converters["DOMString"],
  },
  {
    key: "d",
    converter: webidl.converters["DOMString"],
  },
  {
    key: "t",
    converter: webidl.converters["DOMString"],
  },
];

webidl.converters.RsaOtherPrimesInfo = webidl.createDictionaryConverter(
  "RsaOtherPrimesInfo",
  dictRsaOtherPrimesInfo,
);
webidl.converters["sequence<RsaOtherPrimesInfo>"] = webidl
  .createSequenceConverter(
    webidl.converters.RsaOtherPrimesInfo,
  );

const dictJsonWebKey = [
  // Sections 4.2 and 4.3 of RFC7517.
  // https://datatracker.ietf.org/doc/html/rfc7517#section-4
  {
    key: "kty",
    converter: webidl.converters["DOMString"],
  },
  {
    key: "use",
    converter: webidl.converters["DOMString"],
  },
  {
    key: "key_ops",
    converter: webidl.converters["sequence<DOMString>"],
  },
  {
    key: "alg",
    converter: webidl.converters["DOMString"],
  },
  // JSON Web Key Parameters Registration
  {
    key: "ext",
    converter: webidl.converters["boolean"],
  },
  // Section 6 of RFC7518 JSON Web Algorithms
  // https://datatracker.ietf.org/doc/html/rfc7518#section-6
  {
    key: "crv",
    converter: webidl.converters["DOMString"],
  },
  {
    key: "x",
    converter: webidl.converters["DOMString"],
  },
  {
    key: "y",
    converter: webidl.converters["DOMString"],
  },
  {
    key: "d",
    converter: webidl.converters["DOMString"],
  },
  {
    key: "n",
    converter: webidl.converters["DOMString"],
  },
  {
    key: "e",
    converter: webidl.converters["DOMString"],
  },
  {
    key: "p",
    converter: webidl.converters["DOMString"],
  },
  {
    key: "q",
    converter: webidl.converters["DOMString"],
  },
  {
    key: "dp",
    converter: webidl.converters["DOMString"],
  },
  {
    key: "dq",
    converter: webidl.converters["DOMString"],
  },
  {
    key: "qi",
    converter: webidl.converters["DOMString"],
  },
  {
    key: "oth",
    converter: webidl.converters["sequence<RsaOtherPrimesInfo>"],
  },
  {
    key: "k",
    converter: webidl.converters["DOMString"],
  },
  // AKP (Algorithm Key Pair) key type, used by ML-DSA and other modern
  // algorithms. https://www.rfc-editor.org/rfc/rfc9964
  {
    key: "pub",
    converter: webidl.converters["DOMString"],
  },
  {
    key: "priv",
    converter: webidl.converters["DOMString"],
  },
];

webidl.converters.JsonWebKey = webidl.createDictionaryConverter(
  "JsonWebKey",
  dictJsonWebKey,
);

const dictHkdfParams = [
  ...new SafeArrayIterator(dictAlgorithm),
  {
    key: "hash",
    converter: webidl.converters.HashAlgorithmIdentifier,
    required: true,
  },
  {
    key: "salt",
    converter: webidl.converters["BufferSource"],
    required: true,
  },
  {
    key: "info",
    converter: webidl.converters["BufferSource"],
    required: true,
  },
];

webidl.converters.HkdfParams = webidl
  .createDictionaryConverter("HkdfParams", dictHkdfParams);

const dictPbkdf2Params = [
  ...new SafeArrayIterator(dictAlgorithm),
  {
    key: "hash",
    converter: webidl.converters.HashAlgorithmIdentifier,
    required: true,
  },
  {
    key: "iterations",
    converter: (V, prefix, context, opts) =>
      webidl.converters["unsigned long"](V, prefix, context, {
        ...opts,
        enforceRange: true,
      }),
    required: true,
  },
  {
    key: "salt",
    converter: webidl.converters["BufferSource"],
    required: true,
  },
];

webidl.converters.Pbkdf2Params = webidl
  .createDictionaryConverter("Pbkdf2Params", dictPbkdf2Params);

const dictAesDerivedKeyParams = [
  ...new SafeArrayIterator(dictAlgorithm),
  {
    key: "length",
    converter: (V, prefix, context, opts) =>
      webidl.converters["unsigned long"](V, prefix, context, {
        ...opts,
        enforceRange: true,
      }),
    required: true,
  },
];

const dictAesCbcParams = [
  ...new SafeArrayIterator(dictAlgorithm),
  {
    key: "iv",
    converter: webidl.converters["BufferSource"],
    required: true,
  },
];

const dictAesGcmParams = [
  ...new SafeArrayIterator(dictAlgorithm),
  {
    key: "iv",
    converter: webidl.converters["BufferSource"],
    required: true,
  },
  {
    key: "tagLength",
    converter: (V, prefix, context, opts) =>
      webidl.converters["unsigned long"](V, prefix, context, {
        ...opts,
        enforceRange: true,
      }),
  },
  {
    key: "additionalData",
    converter: webidl.converters["BufferSource"],
  },
];

const dictAesCtrParams = [
  ...new SafeArrayIterator(dictAlgorithm),
  {
    key: "counter",
    converter: webidl.converters["BufferSource"],
    required: true,
  },
  {
    key: "length",
    converter: (V, prefix, context, opts) =>
      webidl.converters["unsigned short"](V, prefix, context, {
        ...opts,
        enforceRange: true,
      }),
    required: true,
  },
];

webidl.converters.AesDerivedKeyParams = webidl
  .createDictionaryConverter("AesDerivedKeyParams", dictAesDerivedKeyParams);

webidl.converters.AesCbcParams = webidl
  .createDictionaryConverter("AesCbcParams", dictAesCbcParams);

webidl.converters.AesGcmParams = webidl
  .createDictionaryConverter("AesGcmParams", dictAesGcmParams);

webidl.converters.AesCtrParams = webidl
  .createDictionaryConverter("AesCtrParams", dictAesCtrParams);

webidl.converters.CryptoKey = webidl.createInterfaceConverter(
  "CryptoKey",
  CryptoKey.prototype,
);

const dictCryptoKeyPair = [
  {
    key: "publicKey",
    converter: webidl.converters.CryptoKey,
  },
  {
    key: "privateKey",
    converter: webidl.converters.CryptoKey,
  },
];

webidl.converters.CryptoKeyPair = webidl
  .createDictionaryConverter("CryptoKeyPair", dictCryptoKeyPair);

const dictEcdhKeyDeriveParams = [
  ...new SafeArrayIterator(dictAlgorithm),
  {
    key: "public",
    converter: webidl.converters.CryptoKey,
    required: true,
  },
];

webidl.converters.EcdhKeyDeriveParams = webidl
  .createDictionaryConverter("EcdhKeyDeriveParams", dictEcdhKeyDeriveParams);

const dictChaCha20Poly1305Params = [
  ...new SafeArrayIterator(dictAlgorithm),
  {
    key: "iv",
    converter: webidl.converters["BufferSource"],
    required: true,
  },
  {
    key: "additionalData",
    converter: webidl.converters["BufferSource"],
  },
];

webidl.converters.ChaCha20Poly1305Params = webidl.createDictionaryConverter(
  "ChaCha20Poly1305Params",
  dictChaCha20Poly1305Params,
);

const dictShakeParams = [
  ...new SafeArrayIterator(dictAlgorithm),
  {
    key: "outputLength",
    converter: (V, prefix, context, opts) =>
      webidl.converters["unsigned long"](V, prefix, context, {
        ...opts,
        enforceRange: true,
      }),
    required: true,
  },
];

webidl.converters.ShakeParams = webidl.createDictionaryConverter(
  "ShakeParams",
  dictShakeParams,
);

const dictCShakeParams = [
  ...new SafeArrayIterator(dictShakeParams),
  {
    key: "functionName",
    converter: webidl.converters["BufferSource"],
  },
  {
    key: "customization",
    converter: webidl.converters["BufferSource"],
  },
];

webidl.converters.CShakeParams = webidl.createDictionaryConverter(
  "CShakeParams",
  dictCShakeParams,
);

const dictTurboShakeParams = [
  ...new SafeArrayIterator(dictShakeParams),
  {
    key: "domainSeparation",
    converter: (V, prefix, context, opts) =>
      webidl.converters["octet"](V, prefix, context, {
        ...opts,
        enforceRange: true,
      }),
  },
];

webidl.converters.TurboShakeParams = webidl.createDictionaryConverter(
  "TurboShakeParams",
  dictTurboShakeParams,
);

const dictMlDsaParams = [
  ...new SafeArrayIterator(dictAlgorithm),
  {
    key: "context",
    converter: webidl.converters["BufferSource"],
  },
];

webidl.converters.MlDsaParams = webidl.createDictionaryConverter(
  "MlDsaParams",
  dictMlDsaParams,
);

// Bridge functions for Node.js KeyObject interop

/**
 * Export CryptoKey data in a format suitable for creating Node.js KeyObject.
 * Returns { type, data } where data is DER bytes (spki/pkcs8) or raw bytes (secret).
 */
function cryptoKeyExportNodeKeyMaterial(cryptoKey) {
  const handle = cryptoKey[_handle];
  const innerKey = getKeyData(handle);
  const type = cryptoKey[_type];
  const algorithmName = cryptoKey[_algorithm].name;

  if (type === "secret") {
    return { type: "secret", data: innerKey.data };
  }

  if (type === "public") {
    let data;
    switch (algorithmName) {
      case "RSASSA-PKCS1-v1_5":
      case "RSA-PSS":
      case "RSA-OAEP":
        data = op_crypto_export_key(
          { algorithm: algorithmName, format: "spki" },
          innerKey,
        );
        break;
      case "ECDH":
      case "ECDSA":
        data = op_crypto_export_key({
          algorithm: algorithmName,
          namedCurve: cryptoKey[_algorithm].namedCurve,
          format: "spki",
        }, innerKey);
        break;
      case "Ed25519":
        data = op_crypto_export_spki_ed25519(innerKey);
        break;
      case "X25519":
        data = op_crypto_export_spki_x25519(innerKey);
        break;
      case "X448":
        data = op_crypto_export_spki_x448(innerKey);
        break;
      case "ML-DSA-44":
      case "ML-DSA-65":
      case "ML-DSA-87":
        data = op_crypto_mldsa_export_spki(
          mldsaVariantId(algorithmName),
          innerKey,
        );
        break;
      default:
        throw new TypeError(`Unsupported algorithm: ${algorithmName}`);
    }
    return { type: "public", data };
  }

  // private
  let data;
  switch (algorithmName) {
    case "RSASSA-PKCS1-v1_5":
    case "RSA-PSS":
    case "RSA-OAEP":
      data = op_crypto_export_key(
        { algorithm: algorithmName, format: "pkcs8" },
        innerKey,
      );
      break;
    case "ECDH":
    case "ECDSA":
      data = op_crypto_export_key({
        algorithm: algorithmName,
        namedCurve: cryptoKey[_algorithm].namedCurve,
        format: "pkcs8",
      }, innerKey);
      break;
    case "Ed25519": {
      data = op_crypto_export_pkcs8_ed25519(
        new Uint8Array([0x04, 0x22, ...new SafeArrayIterator(innerKey)]),
      );
      data[15] = 0x20;
      break;
    }
    case "X25519": {
      data = op_crypto_export_pkcs8_x25519(
        new Uint8Array([0x04, 0x22, ...new SafeArrayIterator(innerKey)]),
      );
      data[15] = 0x20;
      break;
    }
    case "X448": {
      data = op_crypto_export_pkcs8_x448(
        new Uint8Array([0x04, 0x3a, ...new SafeArrayIterator(innerKey)]),
      );
      data[15] = 0x38;
      break;
    }
    case "ML-DSA-44":
    case "ML-DSA-65":
    case "ML-DSA-87":
      if (innerKey?.seed == null) {
        throw new TypeError(
          `Cannot export ${algorithmName} private key without a seed`,
        );
      }
      data = op_crypto_mldsa_export_pkcs8(
        mldsaVariantId(algorithmName),
        innerKey.seed,
      );
      break;
    default:
      throw new TypeError(`Unsupported algorithm: ${algorithmName}`);
  }
  return { type: "private", data };
}

/**
 * Create a CryptoKey from key material (sync). Used by Node.js keyObject.toCryptoKey().
 * @param {string} format - "raw", "spki", or "pkcs8"
 * @param {Uint8Array} keyData - key material
 * @param {object} algorithm - algorithm identifier (string or object)
 * @param {boolean} extractable
 * @param {string[]} usages
 * @returns {CryptoKey}
 */
function importCryptoKeySync(format, keyData, algorithm, extractable, usages) {
  const normalizedAlgorithm = normalizeAlgorithm(algorithm, "importKey");
  const algorithmName = normalizedAlgorithm.name;

  switch (algorithmName) {
    case "HMAC":
      return importKeyHMAC(
        format,
        normalizedAlgorithm,
        keyData,
        extractable,
        usages,
      );
    case "ECDH":
    case "ECDSA":
      return importKeyEC(
        format,
        normalizedAlgorithm,
        keyData,
        extractable,
        usages,
      );
    case "RSASSA-PKCS1-v1_5":
    case "RSA-PSS":
    case "RSA-OAEP":
      return importKeyRSA(
        format,
        normalizedAlgorithm,
        keyData,
        extractable,
        usages,
      );
    case "HKDF":
      return importKeyHKDF(format, keyData, extractable, usages);
    case "PBKDF2":
      return importKeyPBKDF2(format, keyData, extractable, usages);
    case "AES-CTR":
    case "AES-CBC":
    case "AES-GCM":
    case "AES-OCB":
      return importKeyAES(
        format,
        normalizedAlgorithm,
        keyData,
        extractable,
        usages,
        ["encrypt", "decrypt", "wrapKey", "unwrapKey"],
      );
    case "AES-KW":
      return importKeyAES(
        format,
        normalizedAlgorithm,
        keyData,
        extractable,
        usages,
        ["wrapKey", "unwrapKey"],
      );
    case "ChaCha20-Poly1305":
      // The node:crypto interop boundary uses "raw" for secret key bytes;
      // ChaCha20-Poly1305 only recognizes the unified "raw-secret" format.
      return importKeyChaCha20Poly1305(
        format === "raw" ? "raw-secret" : format,
        keyData,
        extractable,
        usages,
      );
    case "X448":
      return importKeyX448(format, keyData, extractable, usages);
    case "X25519":
      return importKeyX25519(format, keyData, extractable, usages);
    case "Ed25519":
      return importKeyEd25519(format, keyData, extractable, usages);
    case "ML-DSA-44":
    case "ML-DSA-65":
    case "ML-DSA-87":
      return importKeyMlDsa(
        format,
        normalizedAlgorithm,
        keyData,
        extractable,
        usages,
      );
    default:
      throw new DOMException("Not implemented", "NotSupportedError");
  }
}

return {
  Crypto,
  crypto,
  CryptoKey,
  cryptoKeyExportNodeKeyMaterial,
  importCryptoKeySync,
  SubtleCrypto,
};
})();

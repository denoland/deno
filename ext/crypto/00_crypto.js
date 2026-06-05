// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials, internals } = __bootstrap;
const {
  isArrayBuffer,
  isTypedArray,
  isDataView,
} = core;
const {
  op_crypto_construct_key,
  op_crypto_create_crypto,
  op_crypto_decapsulate_web,
  op_crypto_encapsulate_web,
  op_crypto_export_key,
  op_crypto_export_pkcs8_ed25519,
  op_crypto_export_pkcs8_x25519,
  op_crypto_export_pkcs8_x448,
  op_crypto_export_spki_ed25519,
  op_crypto_export_spki_x25519,
  op_crypto_export_spki_x448,
  op_crypto_generate_key_web,
  op_crypto_get_key_length,
  op_crypto_is_algorithm_registered,
  op_crypto_ml_kem_get_public_key,
  op_crypto_normalize_algorithm,
  op_crypto_mldsa_export_pkcs8,
  op_crypto_mldsa_export_spki,
  op_crypto_unwrap_key_web,
  op_crypto_wrap_key_web,
} = core.ops;
const {
  ArrayBufferIsView,
  ArrayBufferPrototypeGetByteLength,
  ArrayPrototypeFilter,
  ArrayPrototypeIncludes,
  DataViewPrototypeGetBuffer,
  DataViewPrototypeGetByteLength,
  DataViewPrototypeGetByteOffset,
  JSONParse,
  JSONStringify,
  ObjectDefineProperty,
  ObjectKeys,
  ObjectPrototypeIsPrototypeOf,
  SafeArrayIterator,
  StringFromCharCode,
  StringPrototypeCharCodeAt,
  StringPrototypeToUpperCase,
  SymbolFor,
  SyntaxError,
  TypeError,
  TypedArrayPrototypeGetBuffer,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeGetByteOffset,
  TypedArrayPrototypeSlice,
  Uint8Array,
} = primordials;

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const { createFilteredInspectProxy } = core.loadExtScript(
  "ext:deno_web/01_console.js",
);
const { DOMException } = core.loadExtScript(
  "ext:deno_web/01_dom_exception.js",
);
const { kKeyObject } = internals;

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

// See https://www.w3.org/TR/WebCryptoAPI/#dfn-normalize-an-algorithm
// 18.4.4
//
// The WebIDL algorithm-parameter dictionaries, the per-op `supportedAlgorithms`
// registry, and the `simpleAlgorithmDictionaries` BufferSource /
// HashAlgorithmIdentifier member coercion now live in Rust
// (`ext/crypto/web_params.rs`). This op takes the op name + the raw algorithm
// value and returns the normalized algorithm object (with copied BufferSource
// members and `{ name }` hash objects) - the same shape this function used to
// produce.
function normalizeAlgorithm(algorithm, op) {
  return op_crypto_normalize_algorithm(op, algorithm);
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

// `CryptoKey` is a deno_core cppgc (`GarbageCollected`) object implemented in
// Rust (`ext/crypto/web_cryptokey.rs`). It holds the key material + metadata
// directly in Rust, replacing the old JS class and the `KEY_STORE` WeakMap
// that used to map opaque handles to key material. The `type`, `extractable`,
// `algorithm` and `usages` getters live on the Rust object; `keyData` is an
// internal getter exposing the raw key material to the import/export helpers
// and the Node.js `KeyObject` interop.
const CryptoKey = core.ops.CryptoKey;
const CryptoKeyPrototype = CryptoKey.prototype;

// `type` is a JS reserved word so the Rust getter is exposed as `keyType`;
// alias the spec accessor `type` onto the prototype.
ObjectDefineProperty(CryptoKeyPrototype, "type", {
  __proto__: null,
  get() {
    return this.keyType;
  },
  enumerable: true,
  configurable: true,
});

/**
 * Derive the public key associated with this CryptoKey, when the underlying
 * algorithm supports it (currently ML-KEM decapsulation keys and ML-DSA
 * signing keys).
 *
 * https://wicg.github.io/webcrypto-modern-algos/#CryptoKey-method-getPublicKey
 *
 * @returns {CryptoKey}
 */
// Internal: derive the public `CryptoKey` for a private ML-KEM/ML-DSA key.
// Per the WICG modern-algorithms spec `getPublicKey()` lives on
// `SubtleCrypto.prototype`, not on `CryptoKey.prototype`, so this is a plain
// helper rather than a method.
function derivePublicKey(key) {
  if (key.type !== "private") {
    throw new DOMException(
      "getPublicKey() is only valid on private keys",
      "InvalidAccessError",
    );
  }

  const algorithm = key.algorithm;
  const algorithmName = algorithm.name;
  switch (algorithmName) {
    case "ML-KEM-512":
    case "ML-KEM-768":
    case "ML-KEM-1024": {
      // The private key's `keyData` is `{ seed, privateKey }`; the public key
      // is derived from the expanded decapsulation key.
      const privateKeyBytes = key.keyData.privateKey;
      let publicKeyBytes;
      try {
        publicKeyBytes = op_crypto_ml_kem_get_public_key(
          algorithmName,
          privateKeyBytes,
        );
      } catch (_) {
        throw new DOMException(
          "Failed to derive public key",
          "OperationError",
        );
      }
      const filteredUsages = ArrayPrototypeFilter(
        key.usages,
        (u) => u === "encapsulateKey" || u === "encapsulateBits",
      );
      return op_crypto_construct_key(
        "public",
        true,
        filteredUsages.length > 0
          ? filteredUsages
          : ["encapsulateKey", "encapsulateBits"],
        { name: algorithmName },
        publicKeyBytes,
      );
    }
    case "ML-DSA-44":
    case "ML-DSA-65":
    case "ML-DSA-87": {
      // The associated public key is stored on the Rust cppgc CryptoKey
      // (set when the private key was imported/generated).
      const pub = key.mldsaPublicKey;
      if (pub === undefined) {
        throw new DOMException(
          "Public key is not available",
          "InvalidAccessError",
        );
      }
      return pub;
    }
    default:
      throw new DOMException(
        `getPublicKey() is not supported for ${algorithmName}`,
        "NotSupportedError",
      );
  }
}

// Node.js interop: `isCryptoKey()` (ext/node) brand-checks via
// `obj[kKeyObject] !== undefined`. Expose it (returning the raw key material)
// so CryptoKey instances are recognized.
ObjectDefineProperty(CryptoKeyPrototype, kKeyObject, {
  __proto__: null,
  get() {
    return this.keyData;
  },
  enumerable: false,
  configurable: true,
});

// Custom inspect (matches the old JS class output).
ObjectDefineProperty(
  CryptoKeyPrototype,
  SymbolFor("Deno.privateCustomInspect"),
  {
    __proto__: null,
    value: function (inspect, inspectOptions) {
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
    },
    enumerable: false,
    configurable: true,
    writable: true,
  },
);

// Structured-clone support: the host-object brand returns the serialization
// payload built from the Rust getters.
ObjectDefineProperty(CryptoKeyPrototype, core.hostObjectBrand, {
  __proto__: null,
  value: function () {
    return {
      type: "CryptoKey",
      keyType: this.type,
      extractable: this.extractable,
      usages: this.usages,
      algorithm: this.algorithm,
      keyData: this.keyData,
    };
  },
  enumerable: false,
  configurable: false,
  writable: false,
});

core.registerCloneableResource("CryptoKey", (data) => {
  return op_crypto_construct_key(
    data.keyType,
    data.extractable,
    data.usages,
    data.algorithm,
    data.keyData,
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

// Build an `importKey` algorithm dictionary for re-importing a derived RSA
// public key, preserving the hash. Returns null for non-RSA algorithms.
function rsaImportAlgorithm(algorithm) {
  switch (algorithm.name) {
    case "RSASSA-PKCS1-v1_5":
    case "RSA-PSS":
    case "RSA-OAEP":
      return { name: algorithm.name, hash: algorithm.hash };
    default:
      return null;
  }
}

// Build an `importKey` algorithm dictionary for re-importing a derived EC
// public key, preserving the curve. Returns null for non-EC algorithms.
function ecImportAlgorithm(algorithm) {
  switch (algorithm.name) {
    case "ECDSA":
    case "ECDH":
      return { name: algorithm.name, namedCurve: algorithm.namedCurve };
    default:
      return null;
  }
}

function getKeyLength(algorithm) {
  // Ported to Rust: the per-algorithm length validation/computation lives in
  // `op_crypto_get_key_length`, which returns a concrete bit length or null
  // (HKDF/PBKDF2). `null` round-trips as null here.
  return op_crypto_get_key_length({
    name: algorithm.name,
    length: algorithm.length,
    hash: algorithm.hash ? { name: algorithm.hash.name } : undefined,
  });
}

// `SubtleCrypto` is a deno_core cppgc (`GarbageCollected`) object implemented
// in Rust (`ext/crypto/web_subtle.rs`). Most methods (digest/encrypt/decrypt/
// sign/verify/deriveBits/encapsulateBits/decapsulateBits, plus the key
// import/export/generation primitives importKeySync/exportKeySync/
// constructGeneratedKey) live on the Rust object. The thin Promise/webidl
// wrappers below are attached to the `SubtleCrypto.prototype` - mirroring how
// `CryptoKey` aliases `type` / `getPublicKey` onto its prototype.
const SubtleCrypto = core.ops.SubtleCrypto;
const SubtleCryptoPrototype = SubtleCrypto.prototype;

// Methods attached to `SubtleCryptoPrototype` (see the `ObjectDefineProperty`
// wiring after the object literal). `this` is the cppgc `SubtleCrypto` object.
// The per-algorithm key import/export/generation and the cppgc `CryptoKey`
// construction now live in Rust (`ext/crypto/web_keymaker.rs`), exposed on the
// `SubtleCrypto.prototype` as the synchronous `importKeySync`,
// `exportKeySync` and `constructGeneratedKey` methods. The methods below are
// thin wrappers that perform the spec's webidl argument conversion and the
// Promise/orchestration the spec requires; the work happens in Rust.
const subtleCryptoMethods = {
  /**
   * @param {string} format
   * @param {BufferSource} keyData
   * @param {string} algorithm
   * @param {boolean} extractable
   * @param {KeyUsages[]} keyUsages
   * @returns {Promise<any>}
   */
  // deno-lint-ignore require-await
  async importKey(format, keyData, algorithm, extractable, keyUsages) {
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
    extractable = webidl.converters.boolean(
      extractable,
      prefix,
      "Argument 4",
    );
    keyUsages = webidl.converters["sequence<KeyUsage>"](
      keyUsages,
      prefix,
      "Argument 5",
    );
    return this.importKeySync(
      format,
      keyData,
      algorithm,
      extractable,
      keyUsages,
    );
  },

  /**
   * @param {string} format
   * @param {CryptoKey} key
   * @returns {Promise<any>}
   */
  // deno-lint-ignore require-await
  async exportKey(format, key) {
    const prefix = "Failed to execute 'exportKey' on 'SubtleCrypto'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    format = webidl.converters.KeyFormat(format, prefix, "Argument 1");
    key = webidl.converters.CryptoKey(key, prefix, "Argument 2");
    return this.exportKeySync(format, key);
  },

  /**
   * Derive the public key associated with a private `CryptoKey`.
   * https://wicg.github.io/webcrypto-modern-algos/#SubtleCrypto-method-getPublicKey
   *
   * @param {CryptoKey} key
   * @param {KeyUsages[]} keyUsages
   * @returns {Promise<CryptoKey>}
   */
  // deno-lint-ignore require-await
  async getPublicKey(key, keyUsages) {
    const prefix = "Failed to execute 'getPublicKey' on 'SubtleCrypto'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    key = webidl.converters.CryptoKey(key, prefix, "Argument 1");
    keyUsages = webidl.converters["sequence<KeyUsage>"](
      keyUsages,
      prefix,
      "Argument 2",
    );

    const algorithmName = key.algorithm.name;
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

    if (key.type !== "private") {
      throw new DOMException(
        "Public keys can only be derived from private keys",
        "InvalidAccessError",
      );
    }

    // ML-KEM / ML-DSA reuse the `CryptoKey.prototype.getPublicKey()` logic,
    // then narrow the usages to the caller-requested set.
    switch (algorithmName) {
      case "ML-KEM-512":
      case "ML-KEM-768":
      case "ML-KEM-1024":
      case "ML-DSA-44":
      case "ML-DSA-65":
      case "ML-DSA-87": {
        const pub = derivePublicKey(key);
        return this.importKeySync(
          "spki",
          this.exportKeySync("spki", pub),
          { name: algorithmName },
          true,
          keyUsages,
        );
      }
    }

    // Classical algorithms: export the private key as JWK, strip the private
    // components, and re-import the remaining public JWK (which performs the
    // per-algorithm usage validation, rejecting invalid usages).
    const jwk = this.exportKeySync("jwk", key);
    delete jwk.d;
    delete jwk.dp;
    delete jwk.dq;
    delete jwk.q;
    delete jwk.qi;
    delete jwk.p;
    jwk.key_ops = keyUsages;
    const importAlg = rsaImportAlgorithm(key.algorithm) ??
      ecImportAlgorithm(key.algorithm) ?? { name: algorithmName };
    return this.importKeySync("jwk", jwk, importAlg, true, keyUsages);
  },

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

    const normalizedAlgorithm = normalizeAlgorithm(algorithm, "deriveBits");
    const normalizedDerivedKeyAlgorithmLength = normalizeAlgorithm(
      derivedKeyType,
      "get key length",
    );

    if (normalizedAlgorithm.name !== baseKey.algorithm.name) {
      throw new DOMException(
        `Invalid algorithm name: ${normalizedAlgorithm.name}`,
        "InvalidAccessError",
      );
    }
    if (!ArrayPrototypeIncludes(baseKey.usages, "deriveKey")) {
      throw new DOMException(
        "'baseKey' usages does not contain 'deriveKey'",
        "InvalidAccessError",
      );
    }

    const length = getKeyLength(normalizedDerivedKeyAlgorithmLength);
    // `true` selects the internal derive-bits abstract operation, which skips
    // the `deriveBits` usage post-condition (deriveKey only requires the
    // `deriveKey` usage, already validated above).
    const secret = await this.deriveBits(algorithm, baseKey, length, true);
    // Use the unified `raw-secret` format so modern symmetric algorithms (e.g.
    // ChaCha20-Poly1305, which does not recognize the legacy `raw`) work as
    // derived-key targets. For existing symmetric algorithms `raw-secret` is an
    // alias of `raw`.
    const result = this.importKeySync(
      "raw-secret",
      secret,
      derivedKeyType,
      extractable,
      keyUsages,
    );
    if (
      ArrayPrototypeIncludes(["private", "secret"], result.type) &&
      keyUsages.length == 0
    ) {
      throw new SyntaxError("Invalid key usage");
    }
    return result;
  },

  /**
   * @param {string} format
   * @param {CryptoKey} key
   * @param {CryptoKey} wrappingKey
   * @param {AlgorithmIdentifier} wrapAlgorithm
   * @returns {Promise<any>}
   */
  async wrapKey(format, key, wrappingKey, wrapAlgorithm) {
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
      normalizedAlgorithm = normalizeAlgorithm(wrapAlgorithm, "wrapKey");
    } catch (_) {
      normalizedAlgorithm = normalizeAlgorithm(wrapAlgorithm, "encrypt");
    }

    if (normalizedAlgorithm.name !== wrappingKey.algorithm.name) {
      throw new DOMException(
        "Wrapping algorithm does not match key algorithm",
        "InvalidAccessError",
      );
    }
    if (!ArrayPrototypeIncludes(wrappingKey.usages, "wrapKey")) {
      throw new DOMException(
        "The requested operation is not valid for the provided key",
        "InvalidAccessError",
      );
    }
    if (key.extractable === false) {
      throw new DOMException("Key is not extractable", "InvalidAccessError");
    }

    const exportedKey = await this.exportKey(format, key);

    let bytes;
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

    if (normalizedAlgorithm.name === "AES-KW") {
      const cipherText = await op_crypto_wrap_key_web({
        key: wrappingKey.keyData,
      }, bytes);
      return TypedArrayPrototypeGetBuffer(cipherText);
    } else {
      // Construct a new key with ["encrypt"] usage since wrappingKey only has
      // ["wrapKey"].
      return await this.encrypt(
        normalizedAlgorithm,
        op_crypto_construct_key(
          wrappingKey.type,
          wrappingKey.extractable,
          ["encrypt"],
          wrappingKey.algorithm,
          wrappingKey.keyData,
        ),
        bytes,
      );
    }
  },

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
    extractable = webidl.converters.boolean(
      extractable,
      prefix,
      "Argument 6",
    );
    keyUsages = webidl.converters["sequence<KeyUsage>"](
      keyUsages,
      prefix,
      "Argument 7",
    );

    wrappedKey = copyBuffer(wrappedKey);

    let normalizedAlgorithm;
    try {
      normalizedAlgorithm = normalizeAlgorithm(unwrapAlgorithm, "unwrapKey");
    } catch (_) {
      normalizedAlgorithm = normalizeAlgorithm(unwrapAlgorithm, "decrypt");
    }

    if (normalizedAlgorithm.name !== unwrappingKey.algorithm.name) {
      throw new DOMException(
        "Unwrapping algorithm does not match key algorithm",
        "InvalidAccessError",
      );
    }
    if (!ArrayPrototypeIncludes(unwrappingKey.usages, "unwrapKey")) {
      throw new DOMException(
        "The requested operation is not valid for the provided key",
        "InvalidAccessError",
      );
    }

    let key;
    if (normalizedAlgorithm.name === "AES-KW") {
      const plainText = await op_crypto_unwrap_key_web({
        key: unwrappingKey.keyData,
      }, wrappedKey);
      key = TypedArrayPrototypeGetBuffer(plainText);
    } else {
      // Construct a new key with ["decrypt"] usage since unwrappingKey only
      // has ["unwrapKey"].
      key = await this.decrypt(
        normalizedAlgorithm,
        op_crypto_construct_key(
          unwrappingKey.type,
          unwrappingKey.extractable,
          ["decrypt"],
          unwrappingKey.algorithm,
          unwrappingKey.keyData,
        ),
        wrappedKey,
      );
    }

    let bytes;
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

    const result = this.importKeySync(
      format,
      bytes,
      unwrappedKeyAlgorithm,
      extractable,
      keyUsages,
    );
    if (
      (result.type == "secret" || result.type == "private") &&
      keyUsages.length == 0
    ) {
      throw new SyntaxError("Invalid key type");
    }
    result.extractable = extractable;
    result.usages = usageIntersection(keyUsages, recognisedUsages);
    return result;
  },

  /**
   * @param {string} algorithm
   * @param {boolean} extractable
   * @param {KeyUsage[]} keyUsages
   * @returns {Promise<any>}
   */
  async generateKey(algorithm, extractable, keyUsages) {
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

    // The raw key material is generated by the async op; the cppgc
    // `CryptoKey` construction happens in the sync `constructGeneratedKey`.
    const generated = await op_crypto_generate_key_web({
      name: normalizedAlgorithm.name,
      usages,
      modulusLength: normalizedAlgorithm.modulusLength,
      publicExponent: normalizedAlgorithm.publicExponent,
      namedCurve: normalizedAlgorithm.namedCurve,
      length: normalizedAlgorithm.length,
      hash: normalizedAlgorithm.hash
        ? normalizedAlgorithm.hash.name
        : undefined,
    });

    const result = this.constructGeneratedKey({
      name: normalizedAlgorithm.name,
      extractable,
      usages,
      hash: normalizedAlgorithm.hash
        ? normalizedAlgorithm.hash.name
        : undefined,
      modulusLength: normalizedAlgorithm.modulusLength,
      publicExponent: normalizedAlgorithm.publicExponent,
      namedCurve: normalizedAlgorithm.namedCurve,
      length: normalizedAlgorithm.length,
      data: generated.data,
      publicKey: generated.publicKey,
      privateKey: generated.privateKey,
      seed: generated.seed,
    });

    if (ObjectPrototypeIsPrototypeOf(CryptoKeyPrototype, result)) {
      const type = result.type;
      if ((type === "secret" || type === "private") && usages.length === 0) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }
    } else if (
      ObjectPrototypeIsPrototypeOf(CryptoKeyPrototype, result.privateKey)
    ) {
      if (result.privateKey.usages.length === 0) {
        throw new DOMException("Invalid key usage", "SyntaxError");
      }
    }
    return result;
  },

  /**
   * https://wicg.github.io/webcrypto-modern-algos/#SubtleCrypto-method-encapsulateKey
   * @param {AlgorithmIdentifier} algorithm
   * @param {CryptoKey} encapsulationKey
   * @param {AlgorithmIdentifier} sharedKeyAlgorithm
   * @param {boolean} extractable
   * @param {KeyUsage[]} usages
   * @returns {Promise<{ciphertext: ArrayBuffer, sharedKey: CryptoKey}>}
   */
  // deno-lint-ignore require-await
  async encapsulateKey(
    algorithm,
    encapsulationKey,
    sharedKeyAlgorithm,
    extractable,
    usages,
  ) {
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
    extractable = webidl.converters.boolean(
      extractable,
      prefix,
      "Argument 4",
    );
    usages = webidl.converters["sequence<KeyUsage>"](
      usages,
      prefix,
      "Argument 5",
    );

    const normalizedAlgorithm = normalizeAlgorithm(algorithm, "encapsulate");
    const { ciphertext, sharedSecret } = mlKemEncapsulate(
      normalizedAlgorithm,
      encapsulationKey,
      "encapsulateKey",
    );
    const sharedKey = this.importKeySync(
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
  },

  /**
   * https://wicg.github.io/webcrypto-modern-algos/#SubtleCrypto-method-decapsulateKey
   * @param {AlgorithmIdentifier} algorithm
   * @param {CryptoKey} decapsulationKey
   * @param {BufferSource} ciphertext
   * @param {AlgorithmIdentifier} sharedKeyAlgorithm
   * @param {boolean} extractable
   * @param {KeyUsage[]} usages
   * @returns {Promise<CryptoKey>}
   */
  // deno-lint-ignore require-await
  async decapsulateKey(
    algorithm,
    decapsulationKey,
    ciphertext,
    sharedKeyAlgorithm,
    extractable,
    usages,
  ) {
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
    extractable = webidl.converters.boolean(
      extractable,
      prefix,
      "Argument 5",
    );
    usages = webidl.converters["sequence<KeyUsage>"](
      usages,
      prefix,
      "Argument 6",
    );

    ciphertext = copyBuffer(ciphertext);
    const normalizedAlgorithm = normalizeAlgorithm(algorithm, "decapsulate");
    const sharedSecret = mlKemDecapsulate(
      normalizedAlgorithm,
      decapsulationKey,
      ciphertext,
      "decapsulateKey",
    );
    return this.importKeySync(
      "raw",
      sharedSecret,
      sharedKeyAlgorithm,
      extractable,
      usages,
    );
  },
};

// Attach the JS methods (and custom inspect) onto the Rust cppgc
// `SubtleCrypto.prototype`. The `digest` method + constructor live on the Rust
// object already.
for (const name of ObjectKeys(subtleCryptoMethods)) {
  ObjectDefineProperty(SubtleCryptoPrototype, name, {
    __proto__: null,
    value: subtleCryptoMethods[name],
    enumerable: true,
    configurable: true,
    writable: true,
  });
}
ObjectDefineProperty(
  SubtleCryptoPrototype,
  SymbolFor("Deno.privateCustomInspect"),
  {
    __proto__: null,
    value: function (inspect, inspectOptions) {
      return `${this.constructor.name} ${inspect({}, inspectOptions)}`;
    },
    enumerable: false,
    configurable: true,
    writable: true,
  },
);

// https://wicg.github.io/webcrypto-modern-algos/#dom-subtlecrypto-supports
const SUPPORTS_OPERATIONS = [
  "encrypt",
  "decrypt",
  "sign",
  "verify",
  "digest",
  "generateKey",
  "deriveKey",
  "deriveBits",
  "importKey",
  "exportKey",
  "wrapKey",
  "unwrapKey",
  "encapsulateKey",
  "encapsulateBits",
  "decapsulateKey",
  "decapsulateBits",
  "getPublicKey",
];

// Asymmetric algorithms whose private keys carry enough information for
// `CryptoKey.prototype.getPublicKey()` to recover the public key.
const PUBLIC_KEY_DERIVABLE_ALGORITHMS = [
  "RSASSA-PKCS1-v1_5",
  "RSA-PSS",
  "RSA-OAEP",
  "ECDSA",
  "ECDH",
  "Ed25519",
  "X25519",
  "X448",
  "ML-KEM-512",
  "ML-KEM-768",
  "ML-KEM-1024",
  "ML-DSA-44",
  "ML-DSA-65",
  "ML-DSA-87",
];

function supportsGetPublicKey(algName) {
  const upper = StringPrototypeToUpperCase(algName);
  for (let i = 0; i < PUBLIC_KEY_DERIVABLE_ALGORITHMS.length; i++) {
    if (
      StringPrototypeToUpperCase(PUBLIC_KEY_DERIVABLE_ALGORITHMS[i]) === upper
    ) {
      return true;
    }
  }
  return false;
}

// "check support for an algorithm" sub-algorithm. Feature detection only: it
// must answer without validating operation-specific parameter dictionaries, so
// the algorithm name is checked against the Rust `supportedAlgorithms` registry
// (`op_crypto_is_algorithm_registered`) rather than `normalizeAlgorithm`.
function checkSupportForAlgorithm(operation, algorithm, _length) {
  let algName;
  if (typeof algorithm === "string") {
    algName = algorithm;
  } else if (algorithm !== null && typeof algorithm === "object") {
    algName = algorithm.name;
  }
  if (typeof algName !== "string") {
    return false;
  }

  let registeredOp;
  switch (operation) {
    case "encapsulateKey":
    case "encapsulateBits":
      registeredOp = "encapsulate";
      break;
    case "decapsulateKey":
    case "decapsulateBits":
      registeredOp = "decapsulate";
      break;
    case "deriveKey":
      registeredOp = "deriveBits";
      break;
    case "exportKey":
    case "getPublicKey":
      registeredOp = "importKey";
      break;
    default:
      registeredOp = operation;
  }

  if (op_crypto_is_algorithm_registered(registeredOp, algName)) {
    if (operation === "getPublicKey") {
      return supportsGetPublicKey(algName);
    }
    return true;
  }

  // wrapKey / unwrapKey fall back to encrypt / decrypt registrations.
  if (operation === "wrapKey") {
    return op_crypto_is_algorithm_registered("encrypt", algName);
  }
  if (operation === "unwrapKey") {
    return op_crypto_is_algorithm_registered("decrypt", algName);
  }

  return false;
}

function subtleSupports(operation, algorithm, lengthOrHash = undefined) {
  const prefix = "Failed to execute 'supports' on 'SubtleCrypto'";
  webidl.requiredArguments(arguments.length, 2, prefix);
  operation = webidl.converters.DOMString(operation, prefix, "Argument 1");
  algorithm = webidl.converters.AlgorithmIdentifier(
    algorithm,
    prefix,
    "Argument 2",
  );

  if (!ArrayPrototypeIncludes(SUPPORTS_OPERATIONS, operation)) {
    return false;
  }

  let length = null;
  let additionalAlgorithm = null;
  if (lengthOrHash !== undefined && lengthOrHash !== null) {
    if (typeof lengthOrHash === "number") {
      length = lengthOrHash >>> 0;
    } else {
      additionalAlgorithm = lengthOrHash;
    }
  }

  if (additionalAlgorithm !== null) {
    if (
      operation === "deriveKey" || operation === "unwrapKey" ||
      operation === "encapsulateKey" || operation === "decapsulateKey"
    ) {
      if (!checkSupportForAlgorithm("importKey", additionalAlgorithm, null)) {
        return false;
      }
    } else if (operation === "wrapKey") {
      if (!checkSupportForAlgorithm("exportKey", additionalAlgorithm, null)) {
        return false;
      }
    }

    if (operation === "deriveKey") {
      let derivedLen;
      try {
        const normalizedDerived = normalizeAlgorithm(
          additionalAlgorithm,
          "get key length",
        );
        derivedLen = getKeyLength(normalizedDerived);
      } catch {
        return false;
      }
      return checkSupportForAlgorithm("deriveBits", algorithm, derivedLen);
    }
  }

  return checkSupportForAlgorithm(operation, algorithm, length);
}

ObjectDefineProperty(SubtleCrypto, "supports", {
  __proto__: null,
  value: subtleSupports,
  enumerable: true,
  configurable: true,
  writable: true,
});

// The full validation (algorithm-name/type/usage -> InvalidAccessError,
// unknown variant -> NotSupportedError, crypto failure -> OperationError) and
// the ML-KEM encapsulate are performed by `op_crypto_encapsulate_web`. `usage`
// is "encapsulateKey" or "encapsulateBits" so the op checks the right usage.
function mlKemEncapsulate(normalizedAlgorithm, encapsulationKey, usage) {
  const publicKeyBytes = encapsulationKey.keyData;
  const result = op_crypto_encapsulate_web(
    {
      algorithm: normalizedAlgorithm.name,
      keyType: encapsulationKey.type,
      keyUsages: encapsulationKey.usages,
      keyAlgorithmName: encapsulationKey.algorithm.name,
    },
    usage,
    publicKeyBytes,
  );
  return {
    ciphertext: result.ciphertext,
    sharedSecret: result.sharedSecret,
  };
}

// The full validation and the ML-KEM decapsulate (including the ciphertext-size
// check) are performed by `op_crypto_decapsulate_web`. `usage` is
// "decapsulateKey" or "decapsulateBits".
function mlKemDecapsulate(
  normalizedAlgorithm,
  decapsulationKey,
  ciphertext,
  usage,
) {
  // ML-KEM private keys store `{ seed, privateKey }`; the expanded
  // decapsulation key is what the op needs.
  const keyData = decapsulationKey.keyData;
  const privateKeyBytes = keyData != null && keyData.privateKey !== undefined
    ? keyData.privateKey
    : keyData;
  return op_crypto_decapsulate_web(
    {
      algorithm: normalizedAlgorithm.name,
      keyType: decapsulationKey.type,
      keyUsages: decapsulationKey.usages,
      keyAlgorithmName: decapsulationKey.algorithm.name,
    },
    usage,
    privateKeyBytes,
    ciphertext,
  );
}

// `Crypto` and `SubtleCrypto` are deno_core cppgc objects implemented in Rust
// (`ext/crypto/web_subtle.rs`). `Crypto`'s `getRandomValues`, `randomUUID` and
// the `subtle` getter all live on the Rust object. The global `crypto` instance
// is constructed via the internal `op_crypto_create_crypto` op (the public
// constructor throws `IllegalConstructor`).
const Crypto = core.ops.Crypto;
const CryptoPrototype = Crypto.prototype;

// Custom inspect (matches the old JS class output).
ObjectDefineProperty(
  CryptoPrototype,
  SymbolFor("Deno.privateCustomInspect"),
  {
    __proto__: null,
    value: function (inspect, inspectOptions) {
      return inspect(
        createFilteredInspectProxy({
          object: this,
          evaluate: ObjectPrototypeIsPrototypeOf(CryptoPrototype, this),
          keys: ["subtle"],
        }),
        inspectOptions,
      );
    },
    enumerable: false,
    configurable: true,
    writable: true,
  },
);

// The global `crypto` instance is created lazily (the cppgc object cannot be
// constructed at snapshot time, when the C++ heap is unavailable). The getter
// memoizes so `globalThis.crypto` is always the same object (`[SameObject]`).
let cryptoInstance;
function getCrypto() {
  if (cryptoInstance === undefined) {
    cryptoInstance = op_crypto_create_crypto();
  }
  return cryptoInstance;
}

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

// `CryptoKey` is a Rust cppgc object, so it does not carry the webidl `brand`
// symbol that `createInterfaceConverter` checks. Brand-check via the prototype
// chain instead (matching the cppgc `Ref<CryptoKey>` converter semantics).
webidl.converters.CryptoKey = (V, prefix, context) => {
  if (!ObjectPrototypeIsPrototypeOf(CryptoKeyPrototype, V)) {
    throw new TypeError(
      `${prefix}: ${context} is not of type CryptoKey`,
    );
  }
  return V;
};

// Bridge functions for Node.js KeyObject interop

/**
 * Export CryptoKey data in a format suitable for creating Node.js KeyObject.
 * Returns { type, data } where data is DER bytes (spki/pkcs8) or raw bytes (secret).
 */
function cryptoKeyExportNodeKeyMaterial(cryptoKey) {
  const handle = cryptoKey.keyData;
  const innerKey = handle;
  const type = cryptoKey.type;
  const algorithmName = cryptoKey.algorithm.name;

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
          namedCurve: cryptoKey.algorithm.namedCurve,
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
        namedCurve: cryptoKey.algorithm.namedCurve,
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
function importCryptoKeySync(
  format,
  keyData,
  algorithm,
  extractable,
  usages,
) {
  // The per-algorithm import + cppgc CryptoKey construction live in Rust
  // (`SubtleCrypto.importKeySync`).
  return getCrypto().subtle.importKeySync(
    format,
    keyData,
    algorithm,
    extractable,
    usages,
  );
}

return {
  Crypto,
  getCrypto,
  CryptoKey,
  cryptoKeyExportNodeKeyMaterial,
  importCryptoKeySync,
  SubtleCrypto,
};
})();

// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />

"use strict";

((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;
  const { DOMException } = window.__bootstrap.domException;

  const {
    ArrayPrototypeFind,
    ArrayBufferIsView,
    ArrayPrototypeIncludes,
    BigInt64Array,
    StringPrototypeToUpperCase,
    Symbol,
    SymbolFor,
    SymbolToStringTag,
    WeakMap,
    WeakMapPrototypeGet,
    WeakMapPrototypeSet,
    Int8Array,
    Uint8Array,
    TypedArrayPrototypeSlice,
    Int16Array,
    Uint16Array,
    Int32Array,
    Uint32Array,
    Uint8ClampedArray,
    TypeError,
  } = window.__bootstrap.primordials;

  // P-521 is not yet supported.
  const supportedNamedCurves = ["P-256", "P-384"];
  const recognisedUsages = [
    "encrypt",
    "decrypt",
    "sign",
    "verify",
    "deriveKey",
    "deriveBits",
    "wrapKey",
    "unwrapKey",
  ];

  const simpleAlgorithmDictionaries = {
    RsaHashedKeyGenParams: { hash: "HashAlgorithmIdentifier" },
    EcKeyGenParams: {},
    HmacKeyGenParams: { hash: "HashAlgorithmIdentifier" },
    RsaPssParams: {},
    EcdsaParams: { hash: "HashAlgorithmIdentifier" },
    HmacImportParams: { hash: "HashAlgorithmIdentifier" },
  };

  const supportedAlgorithms = {
    "digest": {
      "SHA-1": null,
      "SHA-256": null,
      "SHA-384": null,
      "SHA-512": null,
    },
    "generateKey": {
      "RSASSA-PKCS1-v1_5": "RsaHashedKeyGenParams",
      "RSA-PSS": "RsaHashedKeyGenParams",
      "ECDSA": "EcKeyGenParams",
      "HMAC": "HmacKeyGenParams",
    },
    "sign": {
      "RSASSA-PKCS1-v1_5": null,
      "RSA-PSS": "RsaPssParams",
      "ECDSA": "EcdsaParams",
      "HMAC": null,
    },
    "verify": {
      "RSASSA-PKCS1-v1_5": null,
      "RSA-PSS": "RsaPssParams",
    },
    "importKey": {
      "HMAC": "HmacImportParams",
    },
  };

  // See https://www.w3.org/TR/WebCryptoAPI/#dfn-normalize-an-algorithm
  function normalizeAlgorithm(algorithm, op) {
    if (typeof algorithm == "string") {
      return normalizeAlgorithm({ name: algorithm }, op);
    }

    // 1.
    const registeredAlgorithms = supportedAlgorithms[op];
    // 2. 3.
    const initialAlg = webidl.converters.Algorithm(algorithm, {
      prefix: "Failed to normalize algorithm",
      context: "passed algorithm",
    });
    // 4.
    let algName = initialAlg.name;

    // 5.
    let desiredType = undefined;
    for (const key in registeredAlgorithms) {
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

    const normalizedAlgorithm = webidl.converters[desiredType](algorithm, {
      prefix: "Failed to normalize algorithm",
      context: "passed algorithm",
    });
    normalizedAlgorithm.name = algName;

    const dict = simpleAlgorithmDictionaries[desiredType];
    for (const member in dict) {
      const idlType = dict[member];
      const idlValue = normalizedAlgorithm[member];

      if (idlType === "BufferSource") {
        normalizedAlgorithm[member] = new Uint8Array(
          TypedArrayPrototypeSlice(
            (ArrayBufferIsView(idlValue) ? idlValue.buffer : idlValue),
            idlValue.byteOffset ?? 0,
            idlValue.byteLength,
          ),
        );
      } else if (idlType === "HashAlgorithmIdentifier") {
        normalizedAlgorithm[member] = normalizeAlgorithm(idlValue, "digest");
      } else if (idlType === "AlgorithmIdentifier") {
        // TODO(lucacasonato): implement
        throw new TypeError("unimplemented");
      }
    }

    return normalizedAlgorithm;
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

    constructor() {
      webidl.illegalConstructor();
    }

    /** @returns {string} */
    get type() {
      webidl.assertBranded(this, CryptoKey);
      return this[_type];
    }

    /** @returns {boolean} */
    get extractable() {
      webidl.assertBranded(this, CryptoKey);
      return this[_extractable];
    }

    /** @returns {string[]} */
    get usages() {
      webidl.assertBranded(this, CryptoKey);
      // TODO(lucacasonato): return a SameObject copy
      return this[_usages];
    }

    /** @returns {object} */
    get algorithm() {
      webidl.assertBranded(this, CryptoKey);
      // TODO(lucacasonato): return a SameObject copy
      return this[_algorithm];
    }

    get [SymbolToStringTag]() {
      return "CryptoKey";
    }

    [SymbolFor("Deno.customInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          type: this.type,
          extractable: this.extractable,
          algorithm: this.algorithm,
          usages: this.usages,
        })
      }`;
    }
  }

  webidl.configurePrototype(CryptoKey);

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
    return key;
  }

  // https://w3c.github.io/webcrypto/#concept-usage-intersection
  /**
   * @param {string[]} a
   * @param {string[]} b
   * @returns
   */
  function usageIntersection(a, b) {
    return a.filter((i) => b.includes(i));
  }

  // TODO(lucacasonato): this should be moved to rust
  /** @type {WeakMap<object, object>} */
  const KEY_STORE = new WeakMap();

  class SubtleCrypto {
    constructor() {
      webidl.illegalConstructor();
    }

    /**
     * @param {string} algorithm
     * @param {BufferSource} data
     * @returns {Promise<Uint8Array>}
     */
    async digest(algorithm, data) {
      webidl.assertBranded(this, SubtleCrypto);
      const prefix = "Failed to execute 'digest' on 'SubtleCrypto'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      algorithm = webidl.converters.AlgorithmIdentifier(algorithm, {
        prefix,
        context: "Argument 1",
      });
      data = webidl.converters.BufferSource(data, {
        prefix,
        context: "Argument 2",
      });

      if (ArrayBufferIsView(data)) {
        data = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
      } else {
        data = new Uint8Array(data);
      }

      data = TypedArrayPrototypeSlice(data);

      algorithm = normalizeAlgorithm(algorithm, "digest");

      const result = await core.opAsync(
        "op_crypto_subtle_digest",
        algorithm.name,
        data,
      );

      return result.buffer;
    }

    /**
     * @param {string} algorithm
     * @param {CryptoKey} key
     * @param {BufferSource} data
     * @returns {Promise<any>}
     */
    async sign(algorithm, key, data) {
      webidl.assertBranded(this, SubtleCrypto);
      const prefix = "Failed to execute 'sign' on 'SubtleCrypto'";
      webidl.requiredArguments(arguments.length, 3, { prefix });
      algorithm = webidl.converters.AlgorithmIdentifier(algorithm, {
        prefix,
        context: "Argument 1",
      });
      key = webidl.converters.CryptoKey(key, {
        prefix,
        context: "Argument 2",
      });
      data = webidl.converters.BufferSource(data, {
        prefix,
        context: "Argument 3",
      });

      // 1.
      if (ArrayBufferIsView(data)) {
        data = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
      } else {
        data = new Uint8Array(data);
      }
      data = TypedArrayPrototypeSlice(data);

      // 2.
      const normalizedAlgorithm = normalizeAlgorithm(algorithm, "sign");

      const handle = key[_handle];
      const keyData = WeakMapPrototypeGet(KEY_STORE, handle);

      // 8.
      if (normalizedAlgorithm.name !== key[_algorithm].name) {
        throw new DOMException(
          "Signing algorithm doesn't match key algorithm.",
          "InvalidAccessError",
        );
      }

      // 9.
      if (!ArrayPrototypeIncludes(key[_usages], "sign")) {
        throw new DOMException(
          "Key does not support the 'sign' operation.",
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
          const signature = await core.opAsync("op_crypto_sign_key", {
            key: keyData,
            algorithm: "RSASSA-PKCS1-v1_5",
            hash: hashAlgorithm,
          }, data);

          return signature.buffer;
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
          const signature = await core.opAsync("op_crypto_sign_key", {
            key: keyData,
            algorithm: "RSA-PSS",
            hash: hashAlgorithm,
            saltLength: normalizedAlgorithm.saltLength,
          }, data);

          return signature.buffer;
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

          const signature = await core.opAsync("op_crypto_sign_key", {
            key: keyData,
            algorithm: "ECDSA",
            hash: hashAlgorithm,
            namedCurve,
          }, data);

          return signature.buffer;
        }
        case "HMAC": {
          const hashAlgorithm = key[_algorithm].hash.name;

          const signature = await core.opAsync("op_crypto_sign_key", {
            key: keyData,
            algorithm: "HMAC",
            hash: hashAlgorithm,
          }, data);

          return signature.buffer;
        }
      }

      throw new TypeError("unreachable");
    }

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
      webidl.assertBranded(this, SubtleCrypto);
      const prefix = "Failed to execute 'importKey' on 'SubtleCrypto'";
      webidl.requiredArguments(arguments.length, 4, { prefix });
      format = webidl.converters.KeyFormat(format, {
        prefix,
        context: "Argument 1",
      });
      keyData = webidl.converters.BufferSource(keyData, {
        prefix,
        context: "Argument 2",
      });
      algorithm = webidl.converters.AlgorithmIdentifier(algorithm, {
        prefix,
        context: "Argument 3",
      });
      extractable = webidl.converters.boolean(extractable, {
        prefix,
        context: "Argument 4",
      });
      keyUsages = webidl.converters["sequence<KeyUsage>"](keyUsages, {
        prefix,
        context: "Argument 5",
      });

      const normalizedAlgorithm = normalizeAlgorithm(algorithm, "importKey");

      if (
        ArrayPrototypeFind(
          keyUsages,
          (u) => !ArrayPrototypeIncludes(["sign", "verify"], u),
        ) !== undefined
      ) {
        throw new DOMException("Invalid key usages", "SyntaxError");
      }

      switch (normalizedAlgorithm.name) {
        // https://w3c.github.io/webcrypto/#hmac-operations
        case "HMAC": {
          switch (format) {
            case "raw": {
              const hash = normalizedAlgorithm.hash;
              // 5.
              let length = keyData.byteLength * 8;
              // 6.
              if (length === 0) {
                throw new DOMException("Key length is zero", "DataError");
              }
              if (normalizeAlgorithm.length) {
                // 7.
                if (
                  normalizedAlgorithm.length > length ||
                  normalizedAlgorithm.length <= (length - 8)
                ) {
                  throw new DOMException(
                    "Key length is invalid",
                    "DataError",
                  );
                }
                length = normalizeAlgorithm.length;
              }

              if (keyUsages.length == 0) {
                throw new DOMException("Key usage is empty", "SyntaxError");
              }

              const handle = {};
              WeakMapPrototypeSet(KEY_STORE, handle, {
                type: "raw",
                data: keyData,
              });

              const algorithm = {
                name: "HMAC",
                length,
                hash,
              };

              const key = constructKey(
                "secret",
                true,
                usageIntersection(keyUsages, recognisedUsages),
                algorithm,
                handle,
              );

              return key;
            }
            // TODO(@littledivy): jwk
            default:
              throw new DOMException("Not implemented", "NotSupportedError");
          }
        }
        // TODO(@littledivy): RSASSA-PKCS1-v1_5
        // TODO(@littledivy): RSA-PSS
        // TODO(@littledivy): ECDSA
        default:
          throw new DOMException("Not implemented", "NotSupportedError");
      }
    }

    /**
    * @param {string} format
    * @param {CryptoKey} key
    * @returns {Promise<any>}
    */
    // deno-lint-ignore require-await
    async exportKey(format, key) {
      webidl.assertBranded(this, SubtleCrypto);
      const prefix = "Failed to execute 'exportKey' on 'SubtleCrypto'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      format = webidl.converters.KeyFormat(format, {
        prefix,
        context: "Argument 1",
      });
      key = webidl.converters.CryptoKey(key, {
        prefix,
        context: "Argument 2",
      });

      const handle = key[_handle];
      // 2.
      const bits = WeakMapPrototypeGet(KEY_STORE, handle);

      switch (key[_algorithm].name) {
        case "HMAC": {
          if (bits == null) {
            throw new DOMException("Key is not available", "OperationError");
          }
          switch (format) {
            // 3.
            case "raw": {
              for (let _i = 7 & (8 - bits.length % 8); _i > 0; _i--) {
                bits.push(0);
              }
              // 4-5.
              return bits.buffer;
            }
            // TODO(@littledivy): jwk
            default:
              throw new DOMException("Not implemented", "NotSupportedError");
          }
        }
        // TODO(@littledivy): RSASSA-PKCS1-v1_5
        // TODO(@littledivy): RSA-PSS
        // TODO(@littledivy): ECDSA
        default:
          throw new DOMException("Not implemented", "NotSupportedError");
      }
    }

    /**
     * @param {string} algorithm
     * @param {CryptoKey} key
     * @param {BufferSource} signature
     * @param {BufferSource} data
     * @returns {Promise<boolean>}
     */
    async verify(algorithm, key, signature, data) {
      webidl.assertBranded(this, SubtleCrypto);
      const prefix = "Failed to execute 'verify' on 'SubtleCrypto'";
      webidl.requiredArguments(arguments.length, 4, { prefix });
      algorithm = webidl.converters.AlgorithmIdentifier(algorithm, {
        prefix,
        context: "Argument 1",
      });
      key = webidl.converters.CryptoKey(key, {
        prefix,
        context: "Argument 2",
      });
      signature = webidl.converters.BufferSource(signature, {
        prefix,
        context: "Argument 3",
      });
      data = webidl.converters.BufferSource(data, {
        prefix,
        context: "Argument 4",
      });

      // 2.
      if (ArrayBufferIsView(signature)) {
        signature = new Uint8Array(
          signature.buffer,
          signature.byteOffset,
          signature.byteLength,
        );
      } else {
        signature = new Uint8Array(signature);
      }
      signature = TypedArrayPrototypeSlice(signature);

      // 3.
      if (ArrayBufferIsView(data)) {
        data = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
      } else {
        data = new Uint8Array(data);
      }
      data = TypedArrayPrototypeSlice(data);

      const normalizedAlgorithm = normalizeAlgorithm(algorithm, "verify");

      const handle = key[_handle];
      const keyData = WeakMapPrototypeGet(KEY_STORE, handle);

      if (normalizedAlgorithm.name !== key[_algorithm].name) {
        throw new DOMException(
          "Verifying algorithm doesn't match key algorithm.",
          "InvalidAccessError",
        );
      }

      if (!ArrayPrototypeIncludes(key[_usages], "verify")) {
        throw new DOMException(
          "Key does not support the 'verify' operation.",
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
          return await core.opAsync("op_crypto_verify_key", {
            key: keyData,
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
          const saltLength = normalizedAlgorithm.saltLength;
          return await core.opAsync("op_crypto_verify_key", {
            key: keyData,
            algorithm: "RSA-PSS",
            hash: hashAlgorithm,
            saltLength,
            signature,
          }, data);
        }
      }

      throw new TypeError("unreachable");
    }

    /**
     * @param {string} algorithm
     * @param {boolean} extractable
     * @param {KeyUsage[]} keyUsages
     * @returns {Promise<any>}
     */
    async generateKey(algorithm, extractable, keyUsages) {
      webidl.assertBranded(this, SubtleCrypto);
      const prefix = "Failed to execute 'generateKey' on 'SubtleCrypto'";
      webidl.requiredArguments(arguments.length, 3, { prefix });
      algorithm = webidl.converters.AlgorithmIdentifier(algorithm, {
        prefix,
        context: "Argument 1",
      });
      extractable = webidl.converters["boolean"](extractable, {
        prefix,
        context: "Argument 2",
      });
      keyUsages = webidl.converters["sequence<KeyUsage>"](keyUsages, {
        prefix,
        context: "Argument 3",
      });

      const usages = keyUsages;

      const normalizedAlgorithm = normalizeAlgorithm(algorithm, "generateKey");

      // https://github.com/denoland/deno/pull/9614#issuecomment-866049433
      if (!extractable) {
        throw new DOMException(
          "Non-extractable keys are not supported",
          "SecurityError",
        );
      }

      const result = await generateKey(
        normalizedAlgorithm,
        extractable,
        usages,
      );

      if (result instanceof CryptoKey) {
        const type = result[_type];
        if ((type === "secret" || type === "private") && usages.length === 0) {
          throw new DOMException("Invalid key usages", "SyntaxError");
        }
      } else if (result.privateKey instanceof CryptoKey) {
        if (result.privateKey[_usages].length === 0) {
          throw new DOMException("Invalid key usages", "SyntaxError");
        }
      }

      return result;
    }

    get [SymbolToStringTag]() {
      return "SubtleCrypto";
    }
  }

  async function generateKey(normalizedAlgorithm, extractable, usages) {
    switch (normalizedAlgorithm.name) {
      case "RSASSA-PKCS1-v1_5":
      case "RSA-PSS": {
        // 1.
        if (
          ArrayPrototypeFind(
            usages,
            (u) => !ArrayPrototypeIncludes(["sign", "verify"], u),
          ) !== undefined
        ) {
          throw new DOMException("Invalid key usages", "SyntaxError");
        }

        // 2.
        const keyData = await core.opAsync(
          "op_crypto_generate_key",
          {
            name: normalizedAlgorithm.name,
            modulusLength: normalizedAlgorithm.modulusLength,
            publicExponent: normalizedAlgorithm.publicExponent,
          },
        );
        const handle = {};
        WeakMapPrototypeSet(KEY_STORE, handle, {
          type: "pkcs8",
          data: keyData,
        });

        // 4-8.
        const algorithm = {
          name: normalizedAlgorithm.name,
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
      // TODO(lucacasonato): RSA-OAEP
      case "ECDSA": {
        // 1.
        if (
          ArrayPrototypeFind(
            usages,
            (u) => !ArrayPrototypeIncludes(["sign", "verify"], u),
          ) !== undefined
        ) {
          throw new DOMException("Invalid key usages", "SyntaxError");
        }

        // 2-3.
        const handle = {};
        if (
          ArrayPrototypeIncludes(
            supportedNamedCurves,
            normalizedAlgorithm.namedCurve,
          )
        ) {
          const keyData = await core.opAsync("op_crypto_generate_key", {
            name: "ECDSA",
            namedCurve: normalizedAlgorithm.namedCurve,
          });
          WeakMapPrototypeSet(KEY_STORE, handle, {
            type: "pkcs8",
            data: keyData,
          });
        } else {
          throw new DOMException("Curve not supported", "NotSupportedError");
        }

        // 4-6.
        const algorithm = {
          name: "ECDSA",
          namedCurve: normalizedAlgorithm.namedCurve,
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
      // TODO(lucacasonato): ECDH
      // TODO(lucacasonato): AES-CTR
      // TODO(lucacasonato): AES-CBC
      // TODO(lucacasonato): AES-GCM
      // TODO(lucacasonato): AES-KW
      case "HMAC": {
        // 1.
        if (
          ArrayPrototypeFind(
            usages,
            (u) => !ArrayPrototypeIncludes(["sign", "verify"], u),
          ) !== undefined
        ) {
          throw new DOMException("Invalid key usages", "SyntaxError");
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
        const keyData = await core.opAsync("op_crypto_generate_key", {
          name: "HMAC",
          hash: normalizedAlgorithm.hash.name,
          length,
        });
        const handle = {};
        WeakMapPrototypeSet(KEY_STORE, handle, { type: "raw", data: keyData });

        // 6-10.
        const algorithm = {
          name: "HMAC",
          hash: {
            name: normalizedAlgorithm.hash.name,
          },
          length: keyData.byteLength * 8,
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
    }
  }

  const subtle = webidl.createBranded(SubtleCrypto);

  class Crypto {
    constructor() {
      webidl.illegalConstructor();
    }

    getRandomValues(arrayBufferView) {
      webidl.assertBranded(this, Crypto);
      const prefix = "Failed to execute 'getRandomValues' on 'Crypto'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      arrayBufferView = webidl.converters.ArrayBufferView(arrayBufferView, {
        prefix,
        context: "Argument 1",
      });
      if (
        !(
          arrayBufferView instanceof Int8Array ||
          arrayBufferView instanceof Uint8Array ||
          arrayBufferView instanceof Uint8ClampedArray ||
          arrayBufferView instanceof Int16Array ||
          arrayBufferView instanceof Uint16Array ||
          arrayBufferView instanceof Int32Array ||
          arrayBufferView instanceof Uint32Array ||
          arrayBufferView instanceof BigInt64Array ||
          arrayBufferView instanceof BigUint64Array
        )
      ) {
        throw new DOMException(
          "The provided ArrayBufferView is not an integer array type",
          "TypeMismatchError",
        );
      }
      const ui8 = new Uint8Array(
        arrayBufferView.buffer,
        arrayBufferView.byteOffset,
        arrayBufferView.byteLength,
      );
      core.opSync("op_crypto_get_random_values", ui8);
      return arrayBufferView;
    }

    randomUUID() {
      webidl.assertBranded(this, Crypto);
      return core.opSync("op_crypto_random_uuid");
    }

    get subtle() {
      webidl.assertBranded(this, Crypto);
      return subtle;
    }

    get [SymbolToStringTag]() {
      return "Crypto";
    }

    [SymbolFor("Deno.customInspect")](inspect) {
      return `${this.constructor.name} ${inspect({})}`;
    }
  }

  webidl.configurePrototype(Crypto);

  window.__bootstrap.crypto = {
    SubtleCrypto,
    crypto: webidl.createBranded(Crypto),
    Crypto,
    CryptoKey,
  };
})(this);

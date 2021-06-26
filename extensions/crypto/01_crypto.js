// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;
  const { algDict } = window.__bootstrap.crypto;

  const supportedNamedCurves = ["P-256", "P-384", "P-512"];

  const supportedAlgorithms = {
    "digest": {
      "SHA-1": {},
      "SHA-256": {},
      "SHA-384": {},
      "SHA-512": {},
    },
    "generateKey": {
      "RSASSA-PKCS1-v1_5": "RsaHashedKeyGenParams",
      "RSA-PSS": "RsaHashedKeyGenParams",
      "RSA-OAEP": "RsaHashedKeyGenParams",
      "ECDSA": "EcKeyGenParams",
      "ECDH": "EcKeyGenParams",
      "HMAC": "HmacKeyGenParams",
    },
    "sign": {
      "RSASSA-PKCS1-v1_5": {},
      "RSA-PSS": "RsaPssParams",
      "ECDSA": "EcdsaParams",
      "HMAC": {},
    },
  };

  // See https://www.w3.org/TR/WebCryptoAPI/#dfn-normalize-an-algorithm
  function normalizeAlgorithm(algorithm, op) {
    if (typeof algorithm == "string") {
      return normalizeAlgorithm({ name: algorithm }, op);
    }

    const initialAlgorithm = webidl.converters["Algorithm"](algorithm, {
      context: "Argument 1",
    });

    const registeredAlgorithms = supportedAlgorithms[op];
    const algorithmName = Object.keys(registeredAlgorithms)
      .find((key) => key.toLowerCase() == initialAlgorithm.name.toLowerCase());

    if (algorithmName === undefined) {
      throw new DOMException(
        "Unrecognized algorithm name",
        "NotSupportedError",
      );
    }

    const desiredType = registeredAlgorithms[algorithmName];

    if (typeof desiredType !== "string") {
      return { name: algorithmName };
    }

    const normalizedAlgorithm = webidl.converters[desiredType](algorithm, {});

    normalizedAlgorithm.name = algorithmName;

    if (normalizeAlgorithm.namedCurve) {
      const namedCurve = supportedNamedCurves
        .find((key) =>
          key.toLowerCase() == normalizeAlgorithm.namedCurve.toLowerCase()
        );
      if (namedCurve == undefined) {
        throw new DOMException(
          "namedCurve not supported",
          "NotSupportedError",
        );
      }
    }

    for (const member of algDict[desiredType]) {
      const idlValue = normalizedAlgorithm[member.key];
      if (member.converter == webidl.converters["BufferSource"]) {
        normalizedAlgorithm[member.key] = new Uint8Array(
          ArrayBuffer.isView(idlValue) ? idlValue.buffer : idlValue,
        );
      } else if (
        member.converter == webidl.converters["HashAlgorithmIdentifier"]
      ) {
        normalizedAlgorithm[member.key] = normalizeAlgorithm(
          idlValue,
          "digest",
        );
      } else if (
        member.converter == webidl.converters["AlgorithmIdentifier"]
      ) {
        normalizedAlgorithm[member.key] = normalizeAlgorithm(idlValue, op);
      }
    }

    return normalizedAlgorithm;
  }

  // Should match op_crypto_subtle_digest() in extensions/crypto/lib.rs
  function digestToId(name) {
    switch (name) {
      case "SHA-1":
        return 0;
      case "SHA-256":
        return 1;
      case "SHA-384":
        return 2;
      case "SHA-512":
        return 3;
    }
  }

  const _handle = Symbol("[[handle]]");
  const _algorithm = Symbol("[[algorithm]]");
  const _extractable = Symbol("[[extractable]]");
  const _usages = Symbol("[[usages]]");
  const _type = Symbol("[[type]]");

  class CryptoKey {
    [_usages];
    [_algorithm];
    [_extractable];
    [_type];
    [_handle];

    constructor() {
      webidl.illegalConstructor();
    }

    get usages() {
      return this[_usages];
    }

    get extractable() {
      return this[_extractable];
    }

    get algorithm() {
      return this[_algorithm];
    }

    get type() {
      return this[_type];
    }

    get [Symbol.toStringTag]() {
      return "CryptoKey";
    }

    [Symbol.for("Deno.customInspect")](inspect) {
      return `${this.constructor.name} ${inspect({})}`;
    }
  }

  webidl.configurePrototype(CryptoKey);

  webidl.converters["CryptoKey"] = webidl.createInterfaceConverter(
    "CryptoKey",
    CryptoKey,
  );

  function constructKey(algorithm, extractable, usages, type, handle) {
    const key = webidl.createBranded(CryptoKey);
    key[_algorithm] = algorithm;
    key[_extractable] = extractable;
    key[_usages] = usages;
    key[_type] = type;
    key[_handle] = handle;
    return key;
  }

  // https://w3c.github.io/webcrypto/#concept-usage-intersection
  // TODO(littledivy): When the need arises, make `b` a list.
  function usageIntersection(a, b) {
    return a.includes(b) ? [b] : [];
  }

  const keys = [];

  class SubtleCrypto {
    constructor() {
      webidl.illegalConstructor();
    }

    async digest(algorithm, data) {
      const prefix = "Failed to execute 'digest' on 'SubtleCrypto'";

      webidl.assertBranded(this, SubtleCrypto);
      webidl.requiredArguments(arguments.length, 2);

      algorithm = webidl.converters.AlgorithmIdentifier(algorithm, {
        prefix,
        context: "Argument 1",
      });

      data = webidl.converters.BufferSource(data, {
        prefix,
        context: "Argument 2",
      });

      if (ArrayBuffer.isView(data)) {
        data = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
      } else {
        data = new Uint8Array(data);
      }

      data = data.slice();

      algorithm = normalizeAlgorithm(algorithm, "digest");

      const result = await core.opAsync(
        "op_crypto_subtle_digest",
        digestToId(algorithm.name),
        data,
      );

      return result.buffer;
    }

    async sign(algorithm, key, data) {
      const prefix = "Failed to execute 'sign' on 'SubtleCrypto'";

      webidl.assertBranded(this, SubtleCrypto);
      webidl.requiredArguments(arguments.length, 3);

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

      algorithm = normalizeAlgorithm(algorithm, "sign");

      const index = key[_handle];
      const keyData = keys[index];

      data = new Uint8Array(ArrayBuffer.isView(data) ? data.buffer : data);

      if (algorithm.name == "HMAC") {
        const hashAlgorithm = key[_algorithm].hash.name;

        const signature = await core.opAsync("op_crypto_sign_key", {
          key: keyData,
          algorithm: "HMAC",
          hash: hashAlgorithm,
        }, data);

        return signature;
      } else if (algorithm.name == "ECDSA") {
        // 1.
        if (key[_type] !== "private") {
          throw new DOMException(
            "Key type not supported",
            "InvalidAccessError",
          );
        }

        const namedCurve = key[_algorithm].namedCurve;
        // 2 to 6.
        const signature = await core.opAsync("op_crypto_sign_key", {
          key: keyData,
          algorithm: "ECDSA",
          hash: algorithm.hash.name,
          namedCurve,
        }, data);

        return signature;
      } else if (algorithm.name == "RSA-PSS") {
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
          saltLength: algorithm.saltLength,
        }, data);

        return signature;
      } else if (algorithm.name == "RSASSA-PKCS1-v1_5") {
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

        return signature;
      }
    }

    async generateKey(algorithm, extractable, keyUsages) {
      const prefix = "Failed to execute 'generateKey' on 'SubtleCrypto'";

      webidl.assertBranded(this, SubtleCrypto);
      webidl.requiredArguments(arguments.length, 3);

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

      algorithm = normalizeAlgorithm(algorithm, "generateKey");

      // https://github.com/denoland/deno/pull/9614#issuecomment-866049433
      if (!extractable) {
        throw new DOMException(
          "Extractable keys are not supported",
          "SecurityError",
        );
      }

      if (keyUsages.length == 0) {
        throw new SyntaxError("Usages must not be empty");
      }

      if (algorithm.name == "HMAC") {
        // 1.
        const illegal = keyUsages.find((usage) =>
          !["sign", "verify"].includes(usage)
        );
        if (illegal) {
          throw new SyntaxError("Invalid usage");
        }

        // 2.
        if (algorithm.length == 0) {
          throw new DOMException("Invalid key length", "OperationError");
        }

        // 3.
        const rawMaterial = await core.opAsync(
          "op_crypto_generate_key",
          {
            ...algorithm,
            hash: algorithm.hash.name,
          },
        );
        const index = keys.push({ type: "raw", data: rawMaterial }) - 1;

        // 11 to 13.
        const key = constructKey(
          {
            name: "HMAC",
            hash: algorithm.hash,
            length: algorithm.length,
          },
          extractable,
          keyUsages,
          "secret",
          index,
        );

        // 14.
        return key;
      } else if (algorithm.name == "ECDSA") {
        // 1.
        const illegal = keyUsages.find((usage) =>
          !["sign", "verify"].includes(usage)
        );
        if (illegal) {
          throw new SyntaxError("Invalid usage");
        }

        // 3.
        const pkcsMaterial = await core.opAsync(
          "op_crypto_generate_key",
          algorithm,
        );
        const index = keys.push({ type: "pkcs8", data: pkcsMaterial }) - 1;

        const alg = { name: "ECDSA", namedCurve: algorithm.namedCurve };
        const publicKey = constructKey(
          alg,
          extractable,
          usageIntersection(keyUsages, "verify"),
          "public",
          index,
        );
        const privateKey = constructKey(
          alg,
          extractable,
          usageIntersection(keyUsages, "sign"),
          "private",
          index,
        );

        return {
          publicKey,
          privateKey,
        };
      } else if (
        algorithm.name == "RSA-PSS" || algorithm.name == "RSASSA-PKCS1-v1_5"
      ) {
        // 1.
        const illegal = keyUsages.find((usage) =>
          !["sign", "verify"].includes(usage)
        );
        if (illegal) {
          throw new SyntaxError("Invalid usage");
        }

        // 2.
        const pkcsMaterial = await core.opAsync(
          "op_crypto_generate_key",
          {
            ...algorithm,
            hash: algorithm.hash.name,
          },
          algorithm.publicExponent || new Uint8Array(),
        );
        const index = keys.push({ type: "pkcs8", data: pkcsMaterial }) - 1;

        // 4 to 8.
        const alg = {
          name: algorithm.name,
          modulusLength: algorithm.modulusLength,
          publicExponent: algorithm.publicExponent,
          hash: algorithm.hash,
        };
        const publicKey = constructKey(
          alg,
          extractable,
          usageIntersection(keyUsages, "verify"),
          "public",
          index,
        );
        const privateKey = constructKey(
          alg,
          extractable,
          usageIntersection(keyUsages, "sign"),
          "private",
          index,
        );

        // 19 to 22.
        return {
          publicKey,
          privateKey,
        };
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
          arrayBufferView instanceof Int16Array ||
          arrayBufferView instanceof Uint16Array ||
          arrayBufferView instanceof Int32Array ||
          arrayBufferView instanceof Uint32Array ||
          arrayBufferView instanceof Uint8ClampedArray
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

    get [Symbol.toStringTag]() {
      return "Crypto";
    }

    [Symbol.for("Deno.customInspect")](inspect) {
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

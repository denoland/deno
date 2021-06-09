// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;
  const { algDict } = window.__bootstrap.crypto;

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

    for (const member of algDict[desiredType]) {
      const idlValue = normalizedAlgorithm[member.key];
      if (member.converters == webidl.converters["BufferSource"]) {
        normalizedAlgorithm[member.key] = new Uint8Array(
          ArrayBuffer.isView(idlValue) ? idlValue.buffer : idlValue,
        );
      } else if (
        member.converters == webidl.converters["HashAlgorithmIdentifier"]
      ) {
        normalizedAlgorithm[member.key] = normalizeAlgorithm(
          idlValue,
          "digest",
        );
      } else if (
        member.converters == webidl.converters["AlgorithmIdentifier"]
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

  const ridSymbol = Symbol("[[rid]]");

  class CryptoKey {
    #usages;
    #extractable;
    #algorithm;
    #keyType;

    constructor(key, rid) {
      this.#usages = key.keyUsages;
      this.#extractable = key.extractable;
      const algorithm = key.algorithm;
      let hash = algorithm.hash;
      if (typeof hash == "string") {
        hash = { name: hash };
      }
      this.#algorithm = { ...algorithm, hash };
      this.#keyType = key.keyType;
      this[ridSymbol] = rid;
    }

    get usages() {
      return this.#usages;
    }

    get extractable() {
      return this.#extractable;
    }

    get algorithm() {
      return this.#algorithm;
    }

    get type() {
      return this.#keyType;
    }
  }

  function validateUsages(usages, keyType) {
    let validUsages = [];
    if (keyType == "public") {
      ["encrypt", "verify", "wrapKey"].forEach((usage) => {
        if (usages.includes(usage)) {
          validUsages.push(usage);
        }
      });
    } else if (keyType == "private") {
      ["decrypt", "sign", "unwrapKey", "deriveKey", "deriveBits"].forEach(
        (usage) => {
          if (usages.includes(usage)) {
            validUsages.push(usage);
          }
        },
      );
    } /* secret */ else {
      validUsages = usages;
    }

    return validUsages;
  }

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

    async sign(alg, key, data) {
      const prefix = "Failed to execute 'sign' on 'SubtleCrypto'";

      webidl.assertBranded(this, SubtleCrypto);
      webidl.requiredArguments(arguments.length, 3);

      const rid = key[ridSymbol];

      key = webidl.converters["CryptoKey"](key, {
        prefix,
        context: "Argument 2",
      });

      data = webidl.converters.BufferSource(data, {
        prefix,
        context: "Argument 3",
      });

      alg = normalizeAlgorithm(alg, "sign");

      const { signature } = await core.opAsync("op_webcrypto_sign_key", {
        rid,
        algorithm: alg.name,
        saltLength: alg.saltLength,
        hash: alg.hash,
      }, new Uint8Array(ArrayBuffer.isView(data) ? data.buffer : data));

      return new Uint8Array(signature);
    }

    async generateKey(algorithm, extractable, keyUsages) {
      webidl.assertBranded(this, SubtleCrypto);
      webidl.requiredArguments(arguments.length, 3);

      algorithm = normalizeAlgorithm(algorithm, "generateKey");

      const { key } = await core.opAsync("op_webcrypto_generate_key", {
        algorithm,
        extractable,
        keyUsages,
      }, algorithm.publicExponent || new Uint8Array());

      if (key.single) {
        const { keyType } = key.single.key;
        const usages = validateUsages(keyUsages, keyType);
        return new CryptoKey(
          { keyType, algorithm, extractable, keyUsages: usages },
          key.single.rid,
        );
      } /* CryptoKeyPair */ else {
        const privateKeyType = key.pair.key.privateKey.keyType;
        const privateKeyUsages = validateUsages(keyUsages, privateKeyType);

        const publicKeyType = key.pair.key.publicKey.keyType;
        const publicKeyUsages = validateUsages(keyUsages, publicKeyType);
        return {
          privateKey: new CryptoKey({
            keyType: privateKeyType,
            algorithm,
            extractable,
            keyUsages: privateKeyUsages,
          }, key.pair.private_rid),
          publicKey: new CryptoKey({
            keyType: publicKeyType,
            algorithm,
            extractable: true,
            keyUsages: publicKeyUsages,
          }, key.pair.public_rid),
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

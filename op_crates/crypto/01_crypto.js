// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;

  function getRandomValues(arrayBufferView) {
    if (!ArrayBuffer.isView(arrayBufferView)) {
      throw new TypeError(
        "Argument 1 does not implement interface ArrayBufferView",
      );
    }
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
    if (arrayBufferView.byteLength > 65536) {
      throw new DOMException(
        `The ArrayBufferView's byte length (${arrayBufferView.byteLength}) exceeds the number of bytes of entropy available via this API (65536)`,
        "QuotaExceededError",
      );
    }
    const ui8 = new Uint8Array(
      arrayBufferView.buffer,
      arrayBufferView.byteOffset,
      arrayBufferView.byteLength,
    );
    core.jsonOpSync("op_crypto_get_random_values", null, ui8);
    return arrayBufferView;
  }

  // Represents a rid in a CryptoKey instance.
  const ridSymbol = Symbol();

  // The CryptoKey class. A JavaScript representation of a WebCrypto key.
  // Stores rid of the actual key along with read-only properties.
  class CryptoKey {
    #usages;
    #extractable;
    #algorithm;
    #keyType;

    constructor(key, rid) {
      this.#usages = key.keyUsages;
      this.#extractable = key.extractable;
      const algorithm = key.algorithm;
      algorithm.name = algorithm.name?.toUpperCase();
      if (algorithm.name.startsWith("RSASSA-PKCS1")) {
        // As per spec, `v` cannot be upper case.
        algorithm.name = "RSASSA-PKCS1-v1_5";
      }
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

  // Normalize an algorithm object. makes it less painful to serialize.
  function normalize(algorithm) {
    if (algorithm.publicExponent) {
      if (!(algorithm.publicExponent instanceof Uint8Array)) {
        throw new DOMException(
          "The provided publicExponent is not an Uint8Array",
          "TypeMismatchError",
        );
      }
    }

    const hash = algorithm.hash;
    // Normalizes { hash: { name: "SHA-256" } } to { hash: "SHA-256" }
    if (hash && typeof hash !== "string") {
      hash = hash.name;
    }
    // Algorithm names are not case-sensitive. We use lowercase for internal serialization.
    const name = algorithm.name.toLowerCase();
    return { ...algorithm, name, hash };
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

  async function generateKey(algorithm, extractable, keyUsages) {
    const normalizedAlgorithm = normalize(algorithm);

    const { key, err } = await core.jsonOpAsync("op_webcrypto_generate_key", {
      algorithm: normalizedAlgorithm,
      extractable,
      keyUsages,
    }, normalizedAlgorithm.publicExponent || new Uint8Array());

    // A DOMError.
    if (err) throw new Error(err);

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

  async function sign(alg, key, data) {
    const rid = key[ridSymbol];
    const simpleAlg = typeof alg == "string";
    const saltLength = simpleAlg ? null : alg.saltLength;
    const hash = simpleAlg ? null : alg.hash;
    const algorithm = (simpleAlg ? alg : alg.name).toLowerCase();

    const { signature, err } = await core.jsonOpAsync("op_webcrypto_sign_key", {
      rid,
      algorithm,
      saltLength,
      hash,
    }, data);

    if (err) throw new DOMException(err);
    return new Uint8Array(signature);
  }

  const subtle = {
    generateKey,
    sign,
  };

  window.crypto = {
    getRandomValues,
    subtle,
    CryptoKey,
  };
  window.__bootstrap = window.__bootstrap || {};
  window.__bootstrap.crypto = {
    getRandomValues,
    subtle,
    CryptoKey,
  };
})(this);

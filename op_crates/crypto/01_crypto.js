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
    core.jsonOpSync("op_crypto_get_random_values", {}, ui8);
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
      this.#usages = key.usages;
      this.#extractable = key.extractable;
      this.#algorithm = key.algorithm;
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

    get keyType() {
      return this.#keyType;
    }
  }

  async function generateKey(algorithm, extractable, keyUsages) {
    if (algorithm.publicExponent) {
      if (!(algorithm.publicExponent instanceof Uint8Array)) {
        throw new DOMException(
          "The provided publicExponent is not an Uint8Array",
          "TypeMismatchError",
        );
      }
    }

    const { key, err } = await core.jsonOpAsync("op_webcrypto_generate_key", {
      algorithm,
      extractable,
      keyUsages,
    }, algorithm.publicExponent || null);

    // A DOMError.
    if (err) throw new Error(err);

    return key.single ? new CryptoKey(key.single.key, key.single.rid) : {
      privateKey: new CryptoKey(key.pair.key.privateKey, key.pair.private_rid),
      publicKey: new CryptoKey(key.pair.key.publicKey, key.pair.public_rid),
    };
  }

  async function sign(algorithm, key, data) {
    const rid = key[ridSymbol];
    const simpleParam = typeof algorithm == "string";

    // Normalize params. We've got serde doing the null to Option serialization.
    const saltLength = simpleParam ? null : algorithm.saltLength || null;
    const hash = simpleParam ? null : algorithm.hash || null;
    algorithm = simpleParam ? algorithm : algorithm.name;

    return await core.jsonOpAsync("op_webcrypto_sign_key", {
      rid,
      algorithm,
      saltLength,
      hash,
    }, data).data;
  }

  const subtle = {
    generateKey,
    sign,
  };

  window.crypto = {
    getRandomValues,
    subtle,
  };
  window.__bootstrap = window.__bootstrap || {};
  window.__bootstrap.crypto = {
    getRandomValues,
    generateKey,
    subtle,
  };
})(this);

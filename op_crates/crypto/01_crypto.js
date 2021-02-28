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

  // Just for storing the rid for a crypto key.
  class CryptoKey {    
    constructor(key) {
      this.usages = key.usages;
      this.extractable = key.extractable;
      this.algorithm = key.algorithm;
      this.keyType = key.keyType;
      this.rid = key.rid;
    }
  }

  async function generateKey(algorithm, extractable, keyUsages) {
    return new CryptoKey(await core.jsonOpAsync("op_webcrypto_generate_key", { algorithm, extractable, keyUsages }))
  }

  async function sign(algorithm, key, data) {
    let rid = key.rid;
    return await core.jsonOpAsync("op_webcrypto_sign_key", { rid, algorithm }, data).data;
  }

  window.crypto = {
    getRandomValues,
    subtle: {
      generateKey,
      sign,
    }
  };
  window.__bootstrap = window.__bootstrap || {};
  window.__bootstrap.crypto = {
    getRandomValues,
    generateKey,
    subtle: {
      generateKey,
      sign,
    }
  };
})(this);

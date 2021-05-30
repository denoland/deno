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
    core.opSync("op_crypto_get_random_values", null, ui8);
    return arrayBufferView;
  }

  const subtle = {
    digest(algorithm, data) {
      const digestAlgorithms = [
        "SHA-1",
        "SHA-256",
        "SHA-384",
        "SHA-512",
      ];

      const normalizedAlgorithm = algorithm.toUpperCase();
      const algorithmId = digestAlgorithms.indexOf(normalizedAlgorithm);
      if (algorithmId == -1) {
        throw new DOMException(
          "Unrecognized algorithm name",
          "NotSupportedError",
        );
      }

      return core.opAsync(
        "op_crypto_subtle_digest",
        algorithmId,
        data,
      );
    },
  };

  window.crypto = {
    getRandomValues,
    subtle,
  };
  window.__bootstrap.crypto = {
    getRandomValues,
    subtle,
  };
})(this);

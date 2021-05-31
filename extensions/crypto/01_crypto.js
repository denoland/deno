// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;

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
    async digest(algorithm, data) {
      if (typeof algorithm === "string") {
        algorithm = { name: algorithm };
      } else if (typeof algorithm === "object" && algorithm !== null) {
        if (typeof algorithm.name !== "string") {
          throw new TypeError("Algorithm name is missing or not a string");
        }

        algorithm = { ...algorithm };
      } else {
        throw new TypeError("Argument 1 must be an object or a string");
      }

      data = webidl.converters.BufferSource(data);

      const algorithmName = algorithm.name.toUpperCase();
      const algorithmId = [
        "SHA-1",
        "SHA-256",
        "SHA-384",
        "SHA-512",
      ].indexOf(algorithmName);

      if (algorithmId == -1) {
        throw new DOMException(
          "Unrecognized algorithm name",
          "NotSupportedError",
        );
      }

      const result = await core.opAsync(
        "op_crypto_subtle_digest",
        algorithmId,
        data.slice(),
      );

      return result.buffer;
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

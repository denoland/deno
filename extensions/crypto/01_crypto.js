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

  const supportedAlgorithms = {
    "digest": {
      "SHA-1": {},
      "SHA-256": {},
      "SHA-384": {},
      "SHA-512": {},
    },
  };

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

    // TODO(caspervonb) Step 6 (create from webidl definition), when the need arises.
    // See https://www.w3.org/TR/WebCryptoAPI/#dfn-normalize-an-algorithm
    const normalizedAlgorithm = {};
    normalizedAlgorithm.name = algorithmName;

    // TODO(caspervonb) Step 9 and 10, when the need arises.
    // See https://www.w3.org/TR/WebCryptoAPI/#dfn-normalize-an-algorithm
    return normalizedAlgorithm;
  }

  function serializeAlgorithm(algorithm, op) {
    const registeredAlgorithms = supportedAlgorithms[op];
    const registeredKeys = Object.keys(registeredAlgorithms);

    return registeredKeys.indexOf(algorithm.name);
  }

  const subtle = {
    async digest(algorithm, data) {
      webidl.requiredArguments(arguments.length, 2);

      algorithm = webidl.converters.AlgorithmIdentifier(algorithm, {
        context: "Argument 1",
      });

      data = webidl.converters.BufferSource(data, {
        context: "Argument 2",
      });

      data = data.slice(0);

      algorithm = normalizeAlgorithm(algorithm, "digest");

      const result = await core.opAsync(
        "op_crypto_subtle_digest",
        serializeAlgorithm(algorithm, "digest"),
        data,
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

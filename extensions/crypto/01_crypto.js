// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;

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
  };
})(this);

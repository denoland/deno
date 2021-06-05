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
    const ui8 = new Uint8Array(
      arrayBufferView.buffer,
      arrayBufferView.byteOffset,
      arrayBufferView.byteLength,
    );
    core.opSync("op_crypto_get_random_values", ui8);
    return arrayBufferView;
  }

  function randomUUID() {
    return core.opSync("op_crypto_random_uuid");
  }

  window.crypto = {
    getRandomValues,
    randomUUID,
  };
  window.__bootstrap.crypto = {
    getRandomValues,
    randomUUID,
  };
})(this);

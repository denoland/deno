// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const { sendSync } = window.__bootstrap.dispatchJson;
  const { assert } = window.__bootstrap.util;

  function getRandomValues(typedArray) {
    assert(typedArray !== null, "Input must not be null");
    assert(typedArray.length <= 65536, "Input must not be longer than 65536");
    const ui8 = new Uint8Array(
      typedArray.buffer,
      typedArray.byteOffset,
      typedArray.byteLength,
    );
    sendSync("op_get_random_values", {}, ui8);
    return typedArray;
  }

  window.__bootstrap.crypto = {
    getRandomValues,
  };
})(this);

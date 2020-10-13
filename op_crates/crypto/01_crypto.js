// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = window.Deno.core;
  function getRandomValues(typedArray) {
    if (typedArray == null) throw new Error("Input must not be null");
    if (typedArray.length > 65536) {
      throw new Error("Input must not be longer than 65536");
    }
    const ui8 = new Uint8Array(
      typedArray.buffer,
      typedArray.byteOffset,
      typedArray.byteLength,
    );
    core.jsonOpSync("op_get_random_values", {}, ui8);
    return typedArray;
  }
  window.crypto = {
    getRandomValues,
  };
  window.__bootstrap = window.__bootstrap || {};
  window.__bootstrap.crypto = {
    getRandomValues,
  };
})(this);

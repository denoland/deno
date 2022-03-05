// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference lib="esnext" />

"use strict";

((window) => {
  const webidl = window.__bootstrap.webidl;
  const {
    forgivingBase64Encode,
    forgivingBase64Decode,
  } = window.__bootstrap.infra;
  const { DOMException } = window.__bootstrap.domException;
  const {
    ArrayPrototypeMap,
    StringPrototypeCharCodeAt,
    ArrayPrototypeJoin,
    SafeArrayIterator,
    StringFromCharCode,
    TypedArrayFrom,
    Uint8Array,
  } = window.__bootstrap.primordials;

  /**
   * @param {string} data
   * @returns {string}
   */
  function atob(data) {
    const prefix = "Failed to execute 'atob'";
    webidl.requiredArguments(arguments.length, 1, { prefix });
    data = webidl.converters.DOMString(data, {
      prefix,
      context: "Argument 1",
    });

    return forgivingBase64Decode(data);
  }

  /**
   * @param {string} data
   * @returns {string}
   */
  function btoa(data) {
    const prefix = "Failed to execute 'btoa'";
    webidl.requiredArguments(arguments.length, 1, { prefix });
    data = webidl.converters.DOMString(data, {
      prefix,
      context: "Argument 1",
    });
    return forgivingBase64Encode(data);
  }

  window.__bootstrap.base64 = {
    atob,
    btoa,
  };
})(globalThis);

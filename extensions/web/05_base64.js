// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
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

  /**
   * @param {string} data
   * @returns {string}
   */
  function atob(data) {
    data = webidl.converters.DOMString(data, {
      prefix: "Failed to execute 'atob'",
      context: "Argument 1",
    });

    const uint8Array = forgivingBase64Decode(data);
    const result = [...uint8Array]
      .map((byte) => String.fromCharCode(byte))
      .join("");
    return result;
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
    const byteArray = [...data].map((char) => {
      const charCode = char.charCodeAt(0);
      if (charCode > 0xff) {
        throw new DOMException(
          "The string to be encoded contains characters outside of the Latin1 range.",
          "InvalidCharacterError",
        );
      }
      return charCode;
    });
    return forgivingBase64Encode(Uint8Array.from(byteArray));
  }

  window.__bootstrap.base64 = {
    atob,
    btoa,
  };
})(globalThis);

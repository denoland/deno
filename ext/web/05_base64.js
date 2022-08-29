// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference lib="esnext" />

"use strict";

((window) => {
  const core = Deno.core;
  const ops = core.ops;
  const webidl = window.__bootstrap.webidl;
  const { DOMException } = window.__bootstrap.domException;
  const { TypeError } = window.__bootstrap.primordials;

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
    try {
      return ops.op_base64_atob(data);
    } catch (e) {
      if (e instanceof TypeError) {
        throw new DOMException(
          "Failed to decode base64: invalid character",
          "InvalidCharacterError",
        );
      }
      throw e;
    }
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
    try {
      return ops.op_base64_btoa(data);
    } catch (e) {
      if (e instanceof TypeError) {
        throw new DOMException(
          "The string to be encoded contains characters outside of the Latin1 range.",
          "InvalidCharacterError",
        );
      }
      throw e;
    }
  }

  window.__bootstrap.base64 = {
    atob,
    btoa,
  };
})(globalThis);

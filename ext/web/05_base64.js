// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference lib="esnext" />

"use strict";

((window) => {
  const core = Deno.core;
  const webidl = window.__bootstrap.webidl;

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
    return core.opSync("op_base64_atob", data);
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
    return core.opSync("op_base64_btoa", data);
  }

  window.__bootstrap.base64 = {
    atob,
    btoa,
  };
})(globalThis);

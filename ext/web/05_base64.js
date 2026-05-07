// Copyright 2018-2026 the Deno authors. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference lib="esnext" />

(function () {
const { core, primordials } = globalThis.__bootstrap;
const { op_base64_atob, op_base64_btoa } = core.ops;
const {
  ObjectPrototypeIsPrototypeOf,
  TypeErrorPrototype,
} = primordials;

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const { DOMException } = core.loadExtScript("ext:deno_web/01_dom_exception.js");

/**
 * @param {string} data
 * @returns {string}
 */
function atob(data) {
  const prefix = "Failed to execute 'atob'";
  webidl.requiredArguments(arguments.length, 1, prefix);
  data = webidl.converters.DOMString(data, prefix, "Argument 1");
  try {
    return op_base64_atob(data);
  } catch (e) {
    if (ObjectPrototypeIsPrototypeOf(TypeErrorPrototype, e)) {
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
  webidl.requiredArguments(arguments.length, 1, prefix);
  data = webidl.converters.DOMString(data, prefix, "Argument 1");
  try {
    return op_base64_btoa(data);
  } catch (e) {
    if (ObjectPrototypeIsPrototypeOf(TypeErrorPrototype, e)) {
      throw new DOMException(
        "Cannot encode string: string contains characters outside of the Latin1 range",
        "InvalidCharacterError",
      );
    }
    throw e;
  }
}

return { atob, btoa };
})();

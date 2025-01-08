// Copyright 2018-2025 the Deno authors. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />

import { primordials } from "ext:core/mod.js";
import { op_url_parse_search_params, URL, URLSearchParams } from "ext:core/ops";
const {
  ObjectDefineProperty,
  ObjectPrototypeIsPrototypeOf,
  Symbol,
  SymbolFor,
} = primordials;

import * as webidl from "ext:deno_webidl/00_webidl.js";
import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";

const _list = Symbol("list");

webidl.mixinPairIterable("URLSearchParams", URLSearchParams, _list, 0, 1);

webidl.configureInterface(URLSearchParams);
const URLSearchParamsPrototype = URLSearchParams.prototype;

webidl.converters["URLSearchParams"] = webidl.createInterfaceConverter(
  "URLSearchParams",
  URLSearchParamsPrototype,
);

webidl.configureInterface(URL);
const URLPrototype = URL.prototype;

ObjectDefineProperty(URLPrototype, SymbolFor("Deno.privateCustomInspect"), {
  value(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(URLPrototype, this),
        keys: [
          "href",
          "origin",
          "protocol",
          "username",
          "password",
          "host",
          "hostname",
          "port",
          "pathname",
          "hash",
          "search",
        ],
      }),
      inspectOptions,
    )
  }
});

/**
 * This function implements application/x-www-form-urlencoded parsing.
 * https://url.spec.whatwg.org/#concept-urlencoded-parser
 * @param {Uint8Array} bytes
 * @returns {[string, string][]}
 */
function parseUrlEncoded(bytes) {
  return op_url_parse_search_params(bytes);
}

export {
  parseUrlEncoded,
  URL,
  URLPrototype,
  URLSearchParams,
  URLSearchParamsPrototype,
};

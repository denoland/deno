// Copyright 2018-2025 the Deno authors. MIT license.

// @ts-check
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../url/internal.d.ts" />
/// <reference path="../../cli/tsc/dts/lib.deno_web.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="../web/06_streams_types.d.ts" />
/// <reference path="../../cli/tsc/dts/lib.deno_fetch.d.ts" />
/// <reference lib="esnext" />

import { core, primordials } from "ext:core/mod.js";

import { SymbolDispose } from "ext:deno_web/00_infra.js";
import { op_fetch_custom_client } from "ext:core/ops";
import { loadTlsKeyPair } from "ext:deno_net/02_tls.js";

const { internalRidSymbol } = core;
const {
  JSONStringify,
  ObjectDefineProperty,
  ObjectHasOwn,
  StringPrototypeStartsWith,
  TypeError,
} = primordials;

/**
 * @param {Deno.CreateHttpClientOptions} options
 * @returns {HttpClient}
 */
function createHttpClient(options) {
  options.caCerts ??= [];
  if (options.proxy) {
    if (ObjectHasOwn(options.proxy, "transport")) {
      switch (options.proxy.transport) {
        case "http": {
          const url = options.proxy.url;
          if (
            StringPrototypeStartsWith(url, "https:") ||
            StringPrototypeStartsWith(url, "socks5:") ||
            StringPrototypeStartsWith(url, "socks5h:")
          ) {
            throw new TypeError(
              `The url passed into 'proxy.url' has an invalid scheme for this transport.`,
            );
          }
          options.proxy.transport = "http";
          break;
        }
        case "https": {
          const url = options.proxy.url;
          if (
            StringPrototypeStartsWith(url, "http:") ||
            StringPrototypeStartsWith(url, "socks5:") ||
            StringPrototypeStartsWith(url, "socks5h:")
          ) {
            throw new TypeError(
              `The url passed into 'proxy.url' has an invalid scheme for this transport.`,
            );
          }
          options.proxy.transport = "http";
          break;
        }
        case "socks5": {
          const url = options.proxy.url;
          if (
            !StringPrototypeStartsWith(url, "socks5:") ||
            !StringPrototypeStartsWith(url, "socks5h:")
          ) {
            throw new TypeError(
              `The url passed into 'proxy.url' has an invalid scheme for this transport.`,
            );
          }
          options.proxy.transport = "http";
          break;
        }
        case "unix": {
          break;
        }
        case "vsock": {
          break;
        }
        default: {
          throw new TypeError(
            `Invalid value for 'proxy.transport' option: ${
              JSONStringify(options.proxy.transport)
            }`,
          );
        }
      }
    } else {
      options.proxy.transport = "http";
    }
  }
  const keyPair = loadTlsKeyPair("Deno.createHttpClient", options);
  return new HttpClient(
    op_fetch_custom_client(
      options,
      keyPair,
    ),
  );
}

class HttpClient {
  #rid;

  /**
   * @param {number} rid
   */
  constructor(rid) {
    ObjectDefineProperty(this, internalRidSymbol, {
      __proto__: null,
      enumerable: false,
      value: rid,
    });
    this.#rid = rid;
  }

  close() {
    core.close(this.#rid);
  }

  [SymbolDispose]() {
    core.tryClose(this.#rid);
  }
}
const HttpClientPrototype = HttpClient.prototype;

export { createHttpClient, HttpClient, HttpClientPrototype };

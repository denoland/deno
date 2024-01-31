// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../url/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="../web/06_streams_types.d.ts" />
/// <reference path="./lib.deno_fetch.d.ts" />
/// <reference lib="esnext" />

import { core } from "ext:core/mod.js";

import { SymbolDispose } from "ext:deno_web/00_infra.js";
import { op_fetch_custom_client } from "ext:core/ops";

/**
 * @param {Deno.CreateHttpClientOptions} options
 * @returns {HttpClient}
 */
function createHttpClient(options) {
  options.caCerts ??= [];
  return new HttpClient(
    op_fetch_custom_client(
      options,
    ),
  );
}

class HttpClient {
  #rid;

  /**
   * @param {number} rid
   */
  constructor(rid) {
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

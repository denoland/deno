// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../url/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="../web/06_streams_types.d.ts" />
/// <reference path="./lib.deno_fetch.d.ts" />
/// <reference lib="esnext" />

import { headerListFromHeaders, Headers } from "ext:deno_fetch/20_headers.js";

const core = globalThis.Deno.core;
const ops = core.ops;

/**
 * @param {Deno.CreateHttpClientOptions} options
 * @returns {HttpClient}
 */
function createHttpClient(options) {
  options.caCerts ??= [];
  let defaultHeaders = [];
  if (options.defaultHeaders) {
    defaultHeaders = headerListFromHeaders(new Headers(options.defaultHeaders));
  }
  return new HttpClient(
    ops.op_fetch_custom_client(
      { ...options, defaultHeaders },
    ),
  );
}

class HttpClient {
  /**
   * @param {number} rid
   */
  constructor(rid) {
    this.rid = rid;
  }
  close() {
    core.close(this.rid);
  }
}
const HttpClientPrototype = HttpClient.prototype;

export { createHttpClient, HttpClient, HttpClientPrototype };

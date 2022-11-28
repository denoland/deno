// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../url/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="../web/06_streams_types.d.ts" />
/// <reference path="./lib.deno_fetch.d.ts" />
/// <reference lib="esnext" />
"use strict";

((window) => {
  const core = window.Deno.core;
  const ops = core.ops;

  /**
   * @param {Deno.CreateHttpClientOptions} options
   * @returns {HttpClient}
   */
  function createHttpClient(options) {
    options.caCerts ??= [];
    return new HttpClient(
      ops.op_fetch_custom_client(
        options,
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

  window.__bootstrap.fetch ??= {};
  window.__bootstrap.fetch.createHttpClient = createHttpClient;
  window.__bootstrap.fetch.HttpClient = HttpClient;
  window.__bootstrap.fetch.HttpClientPrototype = HttpClientPrototype;
})(globalThis);

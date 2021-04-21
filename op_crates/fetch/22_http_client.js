// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../url/internal.d.ts" />
/// <reference path="../file/internal.d.ts" />
/// <reference path="../file/lib.deno_file.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="./11_streams_types.d.ts" />
/// <reference path="./lib.deno_fetch.d.ts" />
/// <reference lib="esnext" />
"use strict";

((window) => {
  const core = window.Deno.core;

  /**
   * @param {Deno.CreateHttpClientOptions} options
   * @returns {HttpClient}
   */
  function createHttpClient(options) {
    return new HttpClient(core.opSync("op_create_http_client", options));
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

  window.__bootstrap.fetch ??= {};
  window.__bootstrap.fetch.createHttpClient = createHttpClient;
  window.__bootstrap.fetch.HttpClient = HttpClient;
})(globalThis);

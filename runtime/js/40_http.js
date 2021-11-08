// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.__bootstrap.core;
  const { HttpConn } = window.__bootstrap.http;

  function serveHttp(conn) {
    const rid = core.opSync("op_http_start", conn.rid);
    return new HttpConn(rid);
  }

  window.__bootstrap.http.serveHttp = serveHttp;
})(globalThis);

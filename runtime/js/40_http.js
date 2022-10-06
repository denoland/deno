// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.__bootstrap.core;
  const ops = core.ops;
  const { HttpConn } = window.__bootstrap.http;

  function serveHttp(conn) {
    const rid = ops.op_http_start(conn.rid);
    return new HttpConn(rid, conn.remoteAddr, conn.localAddr);
  }

  window.__bootstrap.http.serveHttp = serveHttp;
})(globalThis);

// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { core } from "ext:core/mod.js";
const ops = core.ops;
import { HttpConn } from "ext:deno_http/01_http.js";

function serveHttp(conn) {
  const rid = ops.op_http_start(conn.rid);
  return new HttpConn(rid, conn.remoteAddr, conn.localAddr);
}

export { serveHttp };

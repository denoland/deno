// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { ops } from "deno:core/01_core.js";
import { HttpConn } from "deno:ext/http/01_http.js";

function serveHttp(conn) {
  const rid = ops.op_http_start(conn.rid);
  return new HttpConn(rid, conn.remoteAddr, conn.localAddr);
}

export { serveHttp };

// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { primordials } from "ext:core/mod.js";
const {
  SymbolAsyncIterator,
} = primordials;
import { serve, serveHttpOnConnection } from "ext:deno_http/00_serve.js";
import { SymbolDispose } from "ext:deno_web/00_infra.js";

class HttpConn {
  #rid = 0;
  #closed = false;
  #remoteAddr;
  #localAddr;
  reqs;
  enqueue;
  server;

  constructor(remoteAddr, localAddr) {
    this.#remoteAddr = remoteAddr;
    this.#localAddr = localAddr;
    const self = this;
    // ReadableStream can be used as a simple async queue. It might not be the
    // most efficient, but this is a deprecated API and we prefer robustness.
    this.reqs = new ReadableStream({
      start(controller) {
        self.enqueue = (request, respondWith) => {
          controller.enqueue({ request, respondWith });
        }
      }
    }).getReader();
  }

  /** @returns {number} */
  get rid() {
    return this.#rid;
  }

  /** @returns {Promise<RequestEvent | null>} */
  async nextRequest() {
    let next = await this.reqs.read();
    if (next.done) {
      return null;
    }
    return next.value;
  }

  /** @returns {void} */
  close() {
    this.server.shutdown()
  }

  [SymbolDispose]() {
    
  }

  [SymbolAsyncIterator]() {
    // deno-lint-ignore no-this-alias
    const httpConn = this;
    return {
      async next() {
        return await httpConn.reqs.read();
      },
    };
  }
}

function serveHttp(conn) {
  const httpConn = new HttpConn();
  const server = serveHttpOnConnection(conn, null, async (req) => {
    const responsePromise = Promise.withResolvers();
    httpConn.enqueue(req, responsePromise.resolve);
    return responsePromise.promise;
  }, (e) => { console.log(e); new Response("error") }, () => {});
  httpConn.server = server;
  return httpConn;
}

const _ws = {};
const upgradeHttp = {};
const upgradeWebSocket = {};
export { _ws, HttpConn, serveHttp, serve, upgradeHttp, upgradeWebSocket };

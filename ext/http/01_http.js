// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";
const {
  Promise,
  PromisePrototypeThen,
  PromisePrototypeCatch,
  SymbolAsyncIterator,
} = primordials;
import { serveHttpOnConnection } from "ext:deno_http/00_serve.js";
import { SymbolDispose } from "ext:deno_web/00_infra.js";
import { AbortController } from "ext:deno_web/03_abort_signal.js";
import {
  _eventLoop,
  _idleTimeoutDuration,
  _idleTimeoutTimeout,
  _protocol,
  _readyState,
  _rid,
  _role,
  _server,
  _serverHandleIdleTimeout,
} from "ext:deno_websocket/01_websocket.js";

class HttpConn {
  #closed = false;
  #remoteAddr;
  #localAddr;
  abortController;
  reqs;
  enqueue;
  closeStream;
  server;
  finished;

  constructor(remoteAddr, localAddr) {
    this.#remoteAddr = remoteAddr;
    this.#localAddr = localAddr;
    this.abortController = new AbortController();
    // deno-lint-ignore no-this-alias
    const self = this;
    // ReadableStream can be used as a simple async queue. It might not be the
    // most efficient, but this is a deprecated API and we prefer robustness.
    this.reqs = new ReadableStream({
      start(controller) {
        self.enqueue = (request, respondWith) => {
          controller.enqueue({ request, respondWith });
        };
        self.closeStream = () => {
          controller.close();
          self.closeStream = () => {};
        };
      },
    }).getReader();
  }

  /** @returns {Promise<RequestEvent | null>} */
  async nextRequest() {
    const next = await this.reqs.read();
    if (next.done) {
      return null;
    }
    return next.value;
  }

  /** @returns {void} */
  async close() {
    this.abortController.abort();
    await this.finished;
  }

  [SymbolDispose]() {
    this.abortController.abort();
    this.closeStream();
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
  const server = serveHttpOnConnection(
    conn,
    httpConn.abortController.signal,
    (req) => {
      let resolver;
      const promise = new Promise((r) => resolver = r);
      httpConn.enqueue(req, resolver);
      return promise;
    },
    (e) => {
      console.log(e);
      new Response("error");
    },
    () => {},
  );
  httpConn.server = server;
  let promise = PromisePrototypeThen(server.finished, () => {
    httpConn.closeStream();
    httpConn.abortController.abort();
    core.tryClose(conn.rid);
  });
  promise = PromisePrototypeCatch(promise, (e) => {
    console.error("Internal HTTP error", e);
    httpConn.closeStream();
    httpConn.abortController.abort();
    core.tryClose(conn.rid);
  });
  httpConn.finished = promise;
  return httpConn;
}

export { HttpConn, serveHttp };

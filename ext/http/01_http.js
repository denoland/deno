// Copyright 2018-2025 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
const {
  BadResourcePrototype,
  InterruptedPrototype,
  internalRidSymbol,
} = core;
import {
  op_http_serve_on,
  op_http_set_promise_complete,
  op_http_set_response_header,
  op_http_set_response_headers,
  op_http_try_wait,
  op_http_wait,
} from "ext:core/ops";
const {
  ObjectPrototypeIsPrototypeOf,
  SymbolAsyncIterator,
  SymbolDispose,
  StringPrototypeIncludes,
  TypeError,
} = primordials;
import { _ws } from "ext:deno_http/02_websocket.ts";
import {
  ResponsePrototype,
  toInnerResponse,
} from "ext:deno_fetch/23_response.js";
import { fromInnerRequest } from "ext:deno_fetch/23_request.js";
import {
  _eventLoop,
  _idleTimeoutDuration,
  _idleTimeoutTimeout,
  _protocol,
  _readyState,
  _rid,
  _role,
  _serverHandleIdleTimeout,
} from "ext:deno_websocket/01_websocket.js";
import {
  CallbackContext,
  fastSyncResponseOrStream,
  InnerRequest,
} from "./00_serve.ts";

class HttpConn {
  #context;

  constructor(context) {
    this.#context = context;
  }

  /** @returns {number} */
  get rid() {
    return this.#context.serverRid;
  }

  /** @returns {Promise<RequestEvent | null>} */
  async nextRequest() {
    let req;
    try {
      req = op_http_try_wait(this.#context.serverRid);
      if (req === null) {
        req = await op_http_wait(this.#context.serverRid);
      }
    } catch (error) {
      this.close();
      if (
        ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error) ||
        ObjectPrototypeIsPrototypeOf(InterruptedPrototype, error) ||
        StringPrototypeIncludes(error.message, "connection closed")
      ) {
        return null;
      }
      throw error;
    }

    if (req === null) {
      this.close();
      return null;
    }

    const innerRequest = new InnerRequest(req, this.#context);
    const request = fromInnerRequest(innerRequest, "immutable");
    innerRequest.request = request;

    const respondWith = createRespondWith(req, innerRequest, this.#context);

    return { request, respondWith };
  }

  /** @returns {void} */
  close() {
    try {
      this.#context.close();
    } catch (error) {
      if (ObjectPrototypeIsPrototypeOf(InterruptedPrototype, error)) {
        return;
      }
      if (ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error)) {
        return;
      }

      throw error;
    }
  }

  [SymbolDispose]() {
    this.close();
  }

  [SymbolAsyncIterator]() {
    // deno-lint-ignore no-this-alias
    const httpConn = this;
    return {
      async next() {
        const reqEvt = await httpConn.nextRequest();
        // Change with caution, current form avoids a v8 deopt
        return { value: reqEvt ?? undefined, done: reqEvt === null };
      },
    };
  }
}

function createRespondWith(req, innerRequest, context) {
  return async function respondWith(response) {
    try {
      response = await response;
      if (!(ObjectPrototypeIsPrototypeOf(ResponsePrototype, response))) {
        throw new TypeError(
          "First argument to 'respondWith' must be a Response or a promise resolving to a Response",
        );
      }

      const inner = toInnerResponse(response);

      if (innerRequest?.upgraded) {
        innerRequest.upgraded();
        return;
      }

      if (context.closed) {
        innerRequest?.close();
        op_http_set_promise_complete(req, 503);
        return;
      }

      const status = inner.status;
      const headers = inner.headerList;
      if (headers && headers.length > 0) {
        if (headers.length == 1) {
          op_http_set_response_header(req, headers[0][0], headers[0][1]);
        } else {
          op_http_set_response_headers(req, headers);
        }
      }

      await fastSyncResponseOrStream(req, inner.body, status, innerRequest);
    } catch (error) {
      innerRequest.close(false);
      throw error;
    }
  };
}

function serveHttp(conn) {
  const context = new CallbackContext(
    null,
    op_http_serve_on(conn[internalRidSymbol]),
    conn,
  );
  return new HttpConn(context);
}

export { HttpConn, serveHttp };

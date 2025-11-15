// Copyright 2018-2025 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
const {
  BadResourcePrototype,
  InterruptedPrototype,
  internalRidSymbol,
} = core;
import {
  op_http_close_after_finish,
  op_http_serve_on,
  op_http_set_promise_complete,
  op_http_set_response_body_legacy,
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
  Symbol,
  TypeError,
  TypedArrayPrototypeGetSymbolToStringTag,
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
  getReadableStreamResourceBacking,
  ReadableStreamPrototype,
  resourceForReadableStream,
} from "ext:deno_web/06_streams.js";
import { CallbackContext, InnerRequest } from "./00_serve.ts";

const connErrorSymbol = Symbol("connError");

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
      // A connection error seen here would cause disrupted responses to throw
      // a generic `BadResource` error. Instead store this error and replace
      // those with it.
      this[connErrorSymbol] = error;
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

    const respondWith = createRespondWith(
      this,
      this.#context,
      req,
      innerRequest,
    );

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

function createRespondWith(httpConn, context, req, innerRequest) {
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

      let staticBody = null;
      let streamRid = 0;
      let streamAutoClose = false;
      if (inner.body != null) {
        const stream = inner.body.streamOrStatic;
        const body = stream.body;
        if (body !== undefined) {
          stream.consumed = true;
        }

        if (
          typeof body === "string" ||
          TypedArrayPrototypeGetSymbolToStringTag(body) === "Uint8Array"
        ) {
          staticBody = body;
        } else {
          if (!ObjectPrototypeIsPrototypeOf(ReadableStreamPrototype, stream)) {
            throw new TypeError("Invalid response");
          }

          const resourceBacking = getReadableStreamResourceBacking(stream);
          if (resourceBacking) {
            streamRid = resourceBacking.rid;
            streamAutoClose = resourceBacking.autoClose;
          } else {
            streamRid = resourceForReadableStream(stream);
            streamAutoClose = true;
          }
        }
      }

      try {
        const consumed = await op_http_set_response_body_legacy(
          req,
          context.serverRid,
          status,
          staticBody,
          streamRid,
          streamAutoClose,
        );
        innerRequest.close(consumed);
        if (!consumed) {
          throw core.buildCustomError(
            "Http",
            "The connection closed while writing the response body",
          );
        }
      } finally {
        op_http_close_after_finish(req);
      }
    } catch (error) {
      innerRequest.close(false);

      const connError = httpConn[connErrorSymbol];
      if (
        ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error) &&
        connError != null
      ) {
        // deno-lint-ignore no-ex-assign
        error = new connError.constructor(connError.message);
      }

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

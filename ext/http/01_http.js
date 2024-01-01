// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, internals, primordials } from "ext:core/mod.js";
const { BadResourcePrototype, InterruptedPrototype, ops } = core;
import { InnerBody } from "ext:deno_fetch/22_body.js";
import { Event, setEventTargetData } from "ext:deno_web/02_event.js";
import { BlobPrototype } from "ext:deno_web/09_file.js";
import {
  fromInnerResponse,
  newInnerResponse,
  ResponsePrototype,
  toInnerResponse,
} from "ext:deno_fetch/23_response.js";
import {
  fromInnerRequest,
  newInnerRequest,
  toInnerRequest,
} from "ext:deno_fetch/23_request.js";
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
  createWebSocketBranded,
  SERVER,
  WebSocket,
} from "ext:deno_websocket/01_websocket.js";
import { TcpConn, UnixConn } from "ext:deno_net/01_net.js";
import { TlsConn } from "ext:deno_net/02_tls.js";
import {
  Deferred,
  getReadableStreamResourceBacking,
  readableStreamClose,
  readableStreamForRid,
  ReadableStreamPrototype,
} from "ext:deno_web/06_streams.js";
import { serve } from "ext:deno_http/00_serve.js";
import { SymbolDispose } from "ext:deno_web/00_infra.js";
const {
  ArrayPrototypeIncludes,
  ArrayPrototypeMap,
  ArrayPrototypePush,
  Error,
  ObjectPrototypeIsPrototypeOf,
  SafeSet,
  SafeSetIterator,
  SetPrototypeAdd,
  SetPrototypeDelete,
  StringPrototypeCharCodeAt,
  StringPrototypeIncludes,
  StringPrototypeSplit,
  StringPrototypeToLowerCase,
  StringPrototypeToUpperCase,
  Symbol,
  SymbolAsyncIterator,
  TypeError,
  Uint8Array,
  Uint8ArrayPrototype,
} = primordials;
const {
  op_http_accept,
  op_http_shutdown,
  op_http_upgrade,
  op_http_write,
  op_http_upgrade_websocket,
  op_http_write_headers,
  op_http_write_resource,
} = core.ensureFastOps();

const connErrorSymbol = Symbol("connError");
const _deferred = Symbol("upgradeHttpDeferred");

/** @type {(self: HttpConn, rid: number) => boolean} */
let deleteManagedResource;

class HttpConn {
  #rid = 0;
  #closed = false;
  #remoteAddr;
  #localAddr;

  // This set holds resource ids of resources
  // that were created during lifecycle of this request.
  // When the connection is closed these resources should be closed
  // as well.
  #managedResources = new SafeSet();

  static {
    deleteManagedResource = (self, rid) =>
      SetPrototypeDelete(self.#managedResources, rid);
  }

  constructor(rid, remoteAddr, localAddr) {
    this.#rid = rid;
    this.#remoteAddr = remoteAddr;
    this.#localAddr = localAddr;
  }

  /** @returns {number} */
  get rid() {
    return this.#rid;
  }

  /** @returns {Promise<RequestEvent | null>} */
  async nextRequest() {
    let nextRequest;
    try {
      nextRequest = await op_http_accept(this.#rid);
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
    if (nextRequest == null) {
      // Work-around for servers (deno_std/http in particular) that call
      // `nextRequest()` before upgrading a previous request which has a
      // `connection: upgrade` header.
      await null;

      this.close();
      return null;
    }

    const { 0: streamRid, 1: method, 2: url } = nextRequest;
    SetPrototypeAdd(this.#managedResources, streamRid);

    /** @type {ReadableStream<Uint8Array> | undefined} */
    let body = null;
    // There might be a body, but we don't expose it for GET/HEAD requests.
    // It will be closed automatically once the request has been handled and
    // the response has been sent.
    if (method !== "GET" && method !== "HEAD") {
      body = readableStreamForRid(streamRid, false);
    }

    const innerRequest = newInnerRequest(
      method,
      url,
      () => ops.op_http_headers(streamRid),
      body !== null ? new InnerBody(body) : null,
      false,
    );
    innerRequest[streamRid] = streamRid;
    const abortController = new AbortController();
    const request = fromInnerRequest(
      innerRequest,
      abortController.signal,
      "immutable",
      false,
    );

    const respondWith = createRespondWith(
      this,
      streamRid,
      request,
      this.#remoteAddr,
      this.#localAddr,
      abortController,
    );

    return { request, respondWith };
  }

  /** @returns {void} */
  close() {
    if (!this.#closed) {
      this.#closed = true;
      core.close(this.#rid);
      for (const rid of new SafeSetIterator(this.#managedResources)) {
        SetPrototypeDelete(this.#managedResources, rid);
        core.close(rid);
      }
    }
  }

  [SymbolDispose]() {
    core.tryClose(this.#rid);
    for (const rid of new SafeSetIterator(this.#managedResources)) {
      SetPrototypeDelete(this.#managedResources, rid);
      core.tryClose(rid);
    }
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

function createRespondWith(
  httpConn,
  streamRid,
  request,
  remoteAddr,
  localAddr,
  abortController,
) {
  return async function respondWith(resp) {
    try {
      resp = await resp;
      if (!(ObjectPrototypeIsPrototypeOf(ResponsePrototype, resp))) {
        throw new TypeError(
          "First argument to respondWith must be a Response or a promise resolving to a Response.",
        );
      }

      const innerResp = toInnerResponse(resp);

      // If response body length is known, it will be sent synchronously in a
      // single op, in other case a "response body" resource will be created and
      // we'll be streaming it.
      /** @type {ReadableStream<Uint8Array> | Uint8Array | null} */
      let respBody = null;
      if (innerResp.body !== null) {
        if (innerResp.body.unusable()) {
          throw new TypeError("Body is unusable.");
        }
        if (
          ObjectPrototypeIsPrototypeOf(
            ReadableStreamPrototype,
            innerResp.body.streamOrStatic,
          )
        ) {
          if (
            innerResp.body.length === null ||
            ObjectPrototypeIsPrototypeOf(
              BlobPrototype,
              innerResp.body.source,
            )
          ) {
            respBody = innerResp.body.stream;
          } else {
            const reader = innerResp.body.stream.getReader();
            const r1 = await reader.read();
            if (r1.done) {
              respBody = new Uint8Array(0);
            } else {
              respBody = r1.value;
              const r2 = await reader.read();
              if (!r2.done) throw new TypeError("Unreachable");
            }
          }
        } else {
          innerResp.body.streamOrStatic.consumed = true;
          respBody = innerResp.body.streamOrStatic.body;
        }
      } else {
        respBody = new Uint8Array(0);
      }
      const isStreamingResponseBody = !(
        typeof respBody === "string" ||
        ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, respBody)
      );
      try {
        await op_http_write_headers(
          streamRid,
          innerResp.status ?? 200,
          innerResp.headerList,
          isStreamingResponseBody ? null : respBody,
        );
      } catch (error) {
        const connError = httpConn[connErrorSymbol];
        if (
          ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error) &&
          connError != null
        ) {
          // deno-lint-ignore no-ex-assign
          error = new connError.constructor(connError.message);
        }
        if (
          respBody !== null &&
          ObjectPrototypeIsPrototypeOf(ReadableStreamPrototype, respBody)
        ) {
          await respBody.cancel(error);
        }
        throw error;
      }

      if (isStreamingResponseBody) {
        let success = false;
        if (
          respBody === null ||
          !ObjectPrototypeIsPrototypeOf(ReadableStreamPrototype, respBody)
        ) {
          throw new TypeError("Unreachable");
        }
        const resourceBacking = getReadableStreamResourceBacking(respBody);
        let reader;
        if (resourceBacking) {
          if (respBody.locked) {
            throw new TypeError("ReadableStream is locked.");
          }
          reader = respBody.getReader(); // Acquire JS lock.
          try {
            await op_http_write_resource(
              streamRid,
              resourceBacking.rid,
            );
            if (resourceBacking.autoClose) core.tryClose(resourceBacking.rid);
            readableStreamClose(respBody); // Release JS lock.
            success = true;
          } catch (error) {
            const connError = httpConn[connErrorSymbol];
            if (
              ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error) &&
              connError != null
            ) {
              // deno-lint-ignore no-ex-assign
              error = new connError.constructor(connError.message);
            }
            await reader.cancel(error);
            throw error;
          }
        } else {
          reader = respBody.getReader();
          while (true) {
            const { value, done } = await reader.read();
            if (done) break;
            if (!ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, value)) {
              await reader.cancel(new TypeError("Value not a Uint8Array"));
              break;
            }
            try {
              await op_http_write(streamRid, value);
            } catch (error) {
              const connError = httpConn[connErrorSymbol];
              if (
                ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error) &&
                connError != null
              ) {
                // deno-lint-ignore no-ex-assign
                error = new connError.constructor(connError.message);
              }
              await reader.cancel(error);
              throw error;
            }
          }
          success = true;
        }

        if (success) {
          try {
            await op_http_shutdown(streamRid);
          } catch (error) {
            await reader.cancel(error);
            throw error;
          }
        }
      }

      const deferred = request[_deferred];
      if (deferred) {
        const res = await op_http_upgrade(streamRid);
        let conn;
        if (res.connType === "tcp") {
          conn = new TcpConn(res.connRid, remoteAddr, localAddr);
        } else if (res.connType === "tls") {
          conn = new TlsConn(res.connRid, remoteAddr, localAddr);
        } else if (res.connType === "unix") {
          conn = new UnixConn(res.connRid, remoteAddr, localAddr);
        } else {
          throw new Error("unreachable");
        }

        deferred.resolve([conn, res.readBuf]);
      }
      const ws = resp[_ws];
      if (ws) {
        const wsRid = await op_http_upgrade_websocket(
          streamRid,
        );
        ws[_rid] = wsRid;
        ws[_protocol] = resp.headers.get("sec-websocket-protocol");

        httpConn.close();

        ws[_readyState] = WebSocket.OPEN;
        ws[_role] = SERVER;
        const event = new Event("open");
        ws.dispatchEvent(event);

        ws[_eventLoop]();
        if (ws[_idleTimeoutDuration]) {
          ws.addEventListener(
            "close",
            () => clearTimeout(ws[_idleTimeoutTimeout]),
          );
        }
        ws[_serverHandleIdleTimeout]();
      }
    } catch (error) {
      abortController.abort(error);
      throw error;
    } finally {
      if (deleteManagedResource(httpConn, streamRid)) {
        core.close(streamRid);
      }
    }
  };
}

const _ws = Symbol("[[associated_ws]]");
const websocketCvf = buildCaseInsensitiveCommaValueFinder("websocket");
const upgradeCvf = buildCaseInsensitiveCommaValueFinder("upgrade");

function upgradeWebSocket(request, options = {}) {
  const inner = toInnerRequest(request);
  const upgrade = request.headers.get("upgrade");
  const upgradeHasWebSocketOption = upgrade !== null &&
    websocketCvf(upgrade);
  if (!upgradeHasWebSocketOption) {
    throw new TypeError(
      "Invalid Header: 'upgrade' header must contain 'websocket'",
    );
  }

  const connection = request.headers.get("connection");
  const connectionHasUpgradeOption = connection !== null &&
    upgradeCvf(connection);
  if (!connectionHasUpgradeOption) {
    throw new TypeError(
      "Invalid Header: 'connection' header must contain 'Upgrade'",
    );
  }

  const websocketKey = request.headers.get("sec-websocket-key");
  if (websocketKey === null) {
    throw new TypeError(
      "Invalid Header: 'sec-websocket-key' header must be set",
    );
  }

  const accept = ops.op_http_websocket_accept_header(websocketKey);

  const r = newInnerResponse(101);
  r.headerList = [
    ["upgrade", "websocket"],
    ["connection", "Upgrade"],
    ["sec-websocket-accept", accept],
  ];

  const protocolsStr = request.headers.get("sec-websocket-protocol") || "";
  const protocols = StringPrototypeSplit(protocolsStr, ", ");
  if (protocols && options.protocol) {
    if (ArrayPrototypeIncludes(protocols, options.protocol)) {
      ArrayPrototypePush(r.headerList, [
        "sec-websocket-protocol",
        options.protocol,
      ]);
    } else {
      throw new TypeError(
        `Protocol '${options.protocol}' not in the request's protocol list (non negotiable)`,
      );
    }
  }

  const socket = createWebSocketBranded(WebSocket);
  setEventTargetData(socket);
  socket[_server] = true;
  socket[_idleTimeoutDuration] = options.idleTimeout ?? 120;
  socket[_idleTimeoutTimeout] = null;

  if (inner._wantsUpgrade) {
    return inner._wantsUpgrade("upgradeWebSocket", r, socket);
  }

  const response = fromInnerResponse(r, "immutable");

  response[_ws] = socket;

  return { response, socket };
}

function upgradeHttp(req) {
  const inner = toInnerRequest(req);
  if (inner._wantsUpgrade) {
    return inner._wantsUpgrade("upgradeHttp", arguments);
  }

  req[_deferred] = new Deferred();
  return req[_deferred].promise;
}

const spaceCharCode = StringPrototypeCharCodeAt(" ", 0);
const tabCharCode = StringPrototypeCharCodeAt("\t", 0);
const commaCharCode = StringPrototypeCharCodeAt(",", 0);

/** Builds a case function that can be used to find a case insensitive
 * value in some text that's separated by commas.
 *
 * This is done because it doesn't require any allocations.
 * @param checkText {string} - The text to find. (ex. "websocket")
 */
function buildCaseInsensitiveCommaValueFinder(checkText) {
  const charCodes = ArrayPrototypeMap(
    StringPrototypeSplit(
      StringPrototypeToLowerCase(checkText),
      "",
    ),
    (c) => [
      StringPrototypeCharCodeAt(c, 0),
      StringPrototypeCharCodeAt(StringPrototypeToUpperCase(c), 0),
    ],
  );
  /** @type {number} */
  let i;
  /** @type {number} */
  let char;

  /** @param {string} value */
  return function (value) {
    for (i = 0; i < value.length; i++) {
      char = StringPrototypeCharCodeAt(value, i);
      skipWhitespace(value);

      if (hasWord(value)) {
        skipWhitespace(value);
        if (i === value.length || char === commaCharCode) {
          return true;
        }
      } else {
        skipUntilComma(value);
      }
    }

    return false;
  };

  /** @param value {string} */
  function hasWord(value) {
    for (let j = 0; j < charCodes.length; ++j) {
      const { 0: cLower, 1: cUpper } = charCodes[j];
      if (cLower === char || cUpper === char) {
        char = StringPrototypeCharCodeAt(value, ++i);
      } else {
        return false;
      }
    }
    return true;
  }

  /** @param value {string} */
  function skipWhitespace(value) {
    while (char === spaceCharCode || char === tabCharCode) {
      char = StringPrototypeCharCodeAt(value, ++i);
    }
  }

  /** @param value {string} */
  function skipUntilComma(value) {
    while (char !== commaCharCode && i < value.length) {
      char = StringPrototypeCharCodeAt(value, ++i);
    }
  }
}

// Expose this function for unit tests
internals.buildCaseInsensitiveCommaValueFinder =
  buildCaseInsensitiveCommaValueFinder;

export { _ws, HttpConn, serve, upgradeHttp, upgradeWebSocket };

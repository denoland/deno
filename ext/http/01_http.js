// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, internals, primordials } from "ext:core/mod.js";
const {
  ArrayPrototypeIncludes,
  ArrayPrototypeMap,
  ArrayPrototypePush,
  StringPrototypeCharCodeAt,
  StringPrototypeSplit,
  StringPrototypeToLowerCase,
  StringPrototypeToUpperCase,
  Promise,
  PromisePrototypeThen,
  SymbolAsyncIterator,
  TypeError,
} = primordials;
import { serve, serveHttpOnConnection } from "ext:deno_http/00_serve.js";
import { SymbolDispose } from "ext:deno_web/00_infra.js";
import { AbortController } from "ext:deno_web/03_abort_signal.js";
import { toInnerRequest } from "ext:deno_fetch/23_request.js";
import {
  fromInnerResponse,
  newInnerResponse,
} from "ext:deno_fetch/23_response.js";
import { Event, setEventTargetData } from "ext:deno_web/02_event.js";
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
  WebSocket,
} from "ext:deno_websocket/01_websocket.js";
const {
  op_http_websocket_accept_header,
} = core.ensureFastOps();

class HttpConn {
  #closed = false;
  #remoteAddr;
  #localAddr;
  abortController;
  reqs;
  enqueue;
  closeStream;
  server;

  constructor(remoteAddr, localAddr) {
    this.#remoteAddr = remoteAddr;
    this.#localAddr = localAddr;
    this.abortController = new AbortController();
    const self = this;
    // ReadableStream can be used as a simple async queue. It might not be the
    // most efficient, but this is a deprecated API and we prefer robustness.
    this.reqs = new ReadableStream({
      start(controller) {
        self.enqueue = (request, respondWith) => {
          controller.enqueue({ request, respondWith });
        };
        self.closeStream = () => {
          try {
            controller.close();
          } catch {}
        };
      },
    }).getReader();
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
  async close() {
    this.abortController.abort();
    await this.server.finished;
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
    async (req) => {
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
  PromisePrototypeThen(server.finished, () => {
    httpConn.closeStream();
    core.tryClose(conn.rid);
  });
  return httpConn;
}

const _ws = {};
const upgradeHttp = {};

const spaceCharCode = StringPrototypeCharCodeAt(" ", 0);
const tabCharCode = StringPrototypeCharCodeAt("\t", 0);
const commaCharCode = StringPrototypeCharCodeAt(",", 0);

const websocketCvf = buildCaseInsensitiveCommaValueFinder("websocket");
const upgradeCvf = buildCaseInsensitiveCommaValueFinder("upgrade");

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

  const accept = op_http_websocket_accept_header(websocketKey);

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

export { _ws, HttpConn, serve, serveHttp, upgradeHttp, upgradeWebSocket };

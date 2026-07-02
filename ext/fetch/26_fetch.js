// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, internals, primordials } = __bootstrap;
const {
  op_fetch,
  op_fetch_promise_is_settled,
  op_fetch_send,
  op_wasm_streaming_set_url,
  op_wasm_streaming_stream_feed,
} = core.ops;
const {
  ArrayPrototypePush,
  ArrayPrototypeSplice,
  ArrayPrototypeFilter,
  ArrayPrototypeIncludes,
  DateNow,
  Error,
  ObjectPrototypeIsPrototypeOf,
  Promise,
  PromisePrototypeThen,
  PromisePrototypeCatch,
  SafeArrayIterator,
  SafePromisePrototypeFinally,
  String,
  StringPrototypeEndsWith,
  StringPrototypeIndexOf,
  StringPrototypeSlice,
  StringPrototypeSplit,
  StringPrototypeStartsWith,
  StringPrototypeToLowerCase,
  StringPrototypeTrim,
  TypeError,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeGetSymbolToStringTag,
} = primordials;

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const { byteLowerCase } = core.loadExtScript("ext:deno_web/00_infra.js");
const {
  acquireReadableStreamDefaultReader,
  errorReadableStream,
  getReadableStreamResourceBacking,
  readableStreamClose,
  readableStreamDisturb,
  readableStreamForRid,
  ReadableStreamPrototype,
  resourceForReadableStream,
} = core.loadExtScript("ext:deno_web/06_streams.js");
const { extractBody, InnerBody } = core.loadExtScript(
  "ext:deno_fetch/22_body.js",
);
const { processUrlList, Request, toInnerRequest } = core.loadExtScript(
  "ext:deno_fetch/23_request.js",
);
const {
  abortedNetworkError,
  fromInnerResponse,
  networkError,
  nullBodyStatus,
  redirectStatus,
  toInnerResponse,
} = core.loadExtScript("ext:deno_fetch/23_response.js");
const abortSignal = core.loadExtScript("ext:deno_web/03_abort_signal.js");
// Make sure both telemetry modules are loaded before destructuring their
// `internals` slots. 99_main.js / 90_deno_ns.js used to load `telemetry.ts`
// unconditionally at snapshot time, but now defer it to actual telemetry
// use. Fetch is the one always-on caller that depends on `__telemetry` and
// `__telemetryUtil`, so it loads both modules on first use.
if (!internals.__telemetry) {
  core.loadExtScript("ext:deno_telemetry/telemetry.ts");
}
if (!internals.__telemetryUtil) {
  core.loadExtScript("ext:deno_telemetry/util.ts");
}
const {
  builtinTracer,
  ContextManager,
  enterSpan,
  restoreSnapshot,
} = internals.__telemetry;
const __telemetry = internals.__telemetry;
const {
  updateSpanFromClientResponse,
  updateSpanFromError,
  updateSpanFromRequest,
} = internals.__telemetryUtil;

const REQUEST_BODY_HEADER_NAMES = [
  "content-encoding",
  "content-language",
  "content-location",
  "content-type",
];

const REDIRECT_SENSITIVE_HEADER_NAMES = [
  "authorization",
  "proxy-authorization",
  "cookie",
];

// https://fetch.spec.whatwg.org/#bad-port
// "bad ports" that fetch must block before any network I/O occurs. Keys are
// the (leading-zero-free) port strings produced by `URL.prototype.port`, so a
// lookup is a single `BAD_PORTS[url.port]` with no parsing. `__proto__: null`
// keeps the lookup safe from prototype pollution.
const BAD_PORTS = {
  __proto__: null,
  "0": true, // (port 0)
  "1": true, // tcpmux
  "7": true, // echo
  "9": true, // discard
  "11": true, // systat
  "13": true, // daytime
  "15": true, // netstat
  "17": true, // qotd
  "19": true, // chargen
  "20": true, // ftp-data
  "21": true, // ftp
  "22": true, // ssh
  "23": true, // telnet
  "25": true, // smtp
  "37": true, // time
  "42": true, // name
  "43": true, // nicname
  "53": true, // domain
  "69": true, // tftp
  "77": true, // priv-rjs
  "79": true, // finger
  "87": true, // ttylink
  "95": true, // supdup
  "101": true, // hostname
  "102": true, // iso-tsap
  "103": true, // gppitnp
  "104": true, // acr-nema
  "109": true, // pop2
  "110": true, // pop3
  "111": true, // sunrpc
  "113": true, // auth
  "115": true, // sftp
  "117": true, // uucp-path
  "119": true, // nntp
  "123": true, // ntp
  "135": true, // loc-srv / epmap
  "137": true, // netbios-ns
  "139": true, // netbios-ssn
  "143": true, // imap2
  "161": true, // snmp
  "179": true, // bgp
  "389": true, // ldap
  "427": true, // svrloc
  "465": true, // submissions
  "512": true, // exec
  "513": true, // login
  "514": true, // shell
  "515": true, // printer
  "526": true, // tempo
  "530": true, // courier
  "531": true, // chat
  "532": true, // netnews
  "540": true, // uucp
  "548": true, // afp
  "554": true, // rtsp
  "556": true, // remotefs
  "563": true, // nntp+ssl
  "587": true, // submission
  "601": true, // syslog-conn
  "636": true, // ldap+ssl
  "989": true, // ftps-data
  "990": true, // ftps
  "993": true, // imap+ssl
  "995": true, // pop3+ssl
  "1719": true, // h323gatestat
  "1720": true, // h323hostcall
  "1723": true, // pptp
  "2049": true, // nfs
  "3659": true, // apple-sasl
  "4045": true, // lockd
  "4190": true, // sieve
  "5060": true, // sip
  "5061": true, // sips
  "6000": true, // x11
  "6566": true, // sane-port
  "6665": true, // ircu
  "6666": true, // ircu
  "6667": true, // ircu
  "6668": true, // ircu
  "6669": true, // ircu
  "6679": true, // osaut
  "6697": true, // ircs-u
  "10080": true, // amanda
};

// ============================================================================
// Inspector Network domain instrumentation (Chrome DevTools Protocol).
//
// When `node:inspector` has been loaded and `--inspect` is active, fetch()
// emits `Network.requestWillBeSent` / `responseReceived` / `dataReceived` /
// `loadingFinished` / `loadingFailed` events. The actual emitters and a
// monotonic requestId generator are installed by `ext/node/polyfills/
// inspector.js` onto `internals.__inspectorNetwork` so this layer doesn't
// have to depend on ext/node.
// ============================================================================

function getInspectorNetwork() {
  const ins = internals.__inspectorNetwork;
  if (ins && ins.isEnabled()) return ins;
  return null;
}

// Inspector emission is purely observational - if anything goes wrong
// (inspector detached mid-flight, payload validation in
// `op_inspector_emit_protocol_event`, etc.) we never want it to surface
// as a fetch error to user code. Centralized so the call sites stay
// straight-line.
function safeEmit(fn, params) {
  try {
    fn(params);
  } catch {
    // swallow
  }
}

// Join repeated header values according to Chrome DevTools conventions:
// cookies use `; `, set-cookie uses `\n`, everything else uses `, `.
//
// Response header names are typically lowercased on the wire by hyper, but
// CDP / Node frontends conventionally key `Set-Cookie` with its canonical
// case (the test suite asserts `headers['Set-Cookie']`). Apply a small
// canonicalization for the names that conventionally carry case.
function joinHeaderValuesForCdp(headerList, lowerCaseNames) {
  const out = { __proto__: null };
  for (let i = 0; i < headerList.length; i++) {
    const rawName = headerList[i][0];
    const value = String(headerList[i][1]);
    const lower = byteLowerCase(rawName);
    let name;
    if (lowerCaseNames) {
      name = lower;
    } else if (lower === "set-cookie") {
      name = "Set-Cookie";
    } else {
      name = rawName;
    }
    let separator;
    if (lower === "cookie") {
      separator = "; ";
    } else if (lower === "set-cookie") {
      separator = "\n";
    } else {
      separator = ", ";
    }
    if (out[name] === undefined) {
      out[name] = value;
    } else {
      out[name] = out[name] + separator + value;
    }
  }
  return out;
}

// Parse Content-Type into { mimeType, charset } for `response.mimeType` and
// `response.charset` (and to decide whether `getResponseBody` returns the
// body as a utf-8 string or base64).
function parseContentTypeForCdp(headerList) {
  let raw = null;
  for (let i = 0; i < headerList.length; i++) {
    if (byteLowerCase(headerList[i][0]) === "content-type") {
      raw = String(headerList[i][1]);
      break;
    }
  }
  if (raw === null) return { mimeType: "", charset: "" };
  const semi = StringPrototypeIndexOf(raw, ";");
  const mimeType = semi === -1
    ? StringPrototypeTrim(raw)
    : StringPrototypeTrim(StringPrototypeSlice(raw, 0, semi));
  let charset = "";
  if (semi !== -1) {
    const rest = StringPrototypeSlice(raw, semi + 1);
    const parts = StringPrototypeSplit(rest, ";");
    for (let i = 0; i < parts.length; i++) {
      const p = StringPrototypeTrim(parts[i]);
      if (
        StringPrototypeStartsWith(StringPrototypeToLowerCase(p), "charset=")
      ) {
        charset = StringPrototypeTrim(StringPrototypeSlice(p, 8));
        // Strip optional surrounding quotes.
        if (
          charset.length >= 2 && charset[0] === '"' &&
          charset[charset.length - 1] === '"'
        ) {
          charset = StringPrototypeSlice(charset, 1, charset.length - 1);
        }
        break;
      }
    }
  }
  return { mimeType, charset };
}

// Run a background drain of the inspector branch of a tee'd response stream,
// emitting `Network.dataReceived` per chunk and `Network.loadingFinished` /
// `loadingFailed` when the stream ends. Errors from this drain are swallowed
// so they can't surface as unhandled rejections to user code.
function drainResponseForInspector(inspectorStream, requestId, ins) {
  const reader = inspectorStream.getReader();
  let totalLength = 0;
  (async () => {
    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        if (value) {
          const len = TypedArrayPrototypeGetByteLength(value);
          if (len > 0) {
            totalLength += len;
            safeEmit(ins.dataReceived, {
              requestId,
              timestamp: DateNow() / 1000,
              dataLength: len,
              encodedDataLength: len,
              data: value,
            });
          }
        }
      }
      safeEmit(ins.loadingFinished, {
        requestId,
        timestamp: DateNow() / 1000,
        encodedDataLength: totalLength,
      });
    } catch (err) {
      safeEmit(ins.loadingFailed, {
        requestId,
        timestamp: DateNow() / 1000,
        type: "Fetch",
        errorText: err && err.message ? String(err.message) : String(err),
      });
    } finally {
      try {
        reader.releaseLock();
      } catch {
        // releaseLock can throw if the stream errored mid-read; swallowing
        // matches what the wrapping `safeEmit` calls do above.
      }
    }
  })();
}

/**
 * @param {number} rid
 * @returns {Promise<{ status: number, statusText: string, headers: [string, string][], url: string, responseRid: number, error: [string, string]? }>}
 */
function opFetchSend(rid) {
  return op_fetch_send(rid);
}

/**
 * @param {number} responseBodyRid
 * @param {AbortSignal} [terminator]
 * @returns {ReadableStream<Uint8Array>}
 */
function createResponseBodyStream(responseBodyRid, terminator) {
  const readable = readableStreamForRid(responseBodyRid);

  function onAbort() {
    errorReadableStream(readable, terminator.reason);
    core.tryClose(responseBodyRid);
  }

  // TODO(lucacasonato): clean up registration
  terminator[abortSignal.add](onAbort);

  return readable;
}

/**
 * @param {InnerRequest} req
 * @param {boolean} recursive
 * @param {AbortSignal} terminator
 * @returns {Promise<InnerResponse>}
 */
async function mainFetch(req, recursive, terminator, inspectorCtx = null) {
  // https://fetch.spec.whatwg.org/#main-fetch step 5: if the request should be
  // blocked due to a bad port, return a network error before any network I/O
  // occurs. This applies to every hop, including those reached via redirects.
  //
  // Per https://fetch.spec.whatwg.org/#block-bad-port the check only applies
  // when the URL's scheme is an HTTP(S) scheme, so https bad ports (e.g.
  // https://example.com:22) are blocked too, while non-HTTP(S) schemes are
  // left alone. The spec's ALPN note covers *new* protocols negotiated over
  // TLS; it does not exempt https fetch from the list. This matches Node's
  // undici (`requestBadPort` gates on `urlIsHttpHttpsScheme`).
  const url = new URL(req.currentUrl());
  if (
    (url.protocol === "http:" || url.protocol === "https:") &&
    url.port !== "" && BAD_PORTS[url.port] === true
  ) {
    return networkError(`Requests to port ${url.port} are blocked`);
  }

  if (req.blobUrlEntry !== null) {
    if (req.method !== "GET") {
      throw new TypeError("Blob URL fetch only supports GET method");
    }

    const body = new InnerBody(req.blobUrlEntry.stream());
    terminator[abortSignal.add](() => body.error(terminator.reason));
    processUrlList(req.urlList, req.urlListProcessed);

    return {
      headerList: [
        ["content-length", String(req.blobUrlEntry.size)],
        ["content-type", req.blobUrlEntry.type],
      ],
      status: 200,
      statusMessage: "OK",
      body,
      type: "basic",
      url() {
        if (this.urlList.length == 0) return null;
        return this.urlList[this.urlList.length - 1];
      },
      urlList: recursive
        ? []
        : [...new SafeArrayIterator(req.urlListProcessed)],
    };
  }

  /** @type {ReadableStream<Uint8Array> | Uint8Array | null} */
  let reqBody = null;
  let reqRid = null;

  if (req.body) {
    const stream = req.body.streamOrStatic;
    const body = stream.body;

    if (TypedArrayPrototypeGetSymbolToStringTag(body) === "Uint8Array") {
      reqBody = body;
    } else if (typeof body === "string") {
      reqBody = core.encode(body);
    } else if (ObjectPrototypeIsPrototypeOf(ReadableStreamPrototype, stream)) {
      const resourceBacking = getReadableStreamResourceBacking(stream);
      if (resourceBacking) {
        reqRid = resourceBacking.rid;
      } else {
        reqRid = resourceForReadableStream(stream, req.body.length);
      }
    } else {
      throw new TypeError("Invalid body");
    }
  }

  const { requestRid, cancelHandleRid } = op_fetch(
    req.method,
    req.currentUrl(),
    req.headerList,
    req.clientRid,
    reqBody !== null || reqRid !== null,
    reqBody,
    reqRid,
  );

  // ---- Inspector: Network.requestWillBeSent ------------------------------
  // Only fires when `node:inspector` is loaded AND the inspector is currently
  // attached, so the cost is one method call (`isEnabled()`) otherwise.
  //
  // For redirect chains the recursive call carries an `inspectorCtx` with the
  // original requestId plus the previous hop's response, so DevTools sees:
  //   requestWillBeSent(URL_A)
  //   requestWillBeSent(URL_B, redirectResponse=<URL_A's 30x>)
  //   responseReceived(URL_B)
  // with the same requestId throughout, matching Chrome's contract.
  const inspectorNetwork = getInspectorNetwork();
  let inspectorRequestId = inspectorCtx?.requestId ?? null;
  const inspectorRedirectResponse = inspectorCtx?.redirectResponse ?? null;
  if (
    inspectorNetwork !== null && inspectorRequestId === null && !recursive
  ) {
    inspectorRequestId = inspectorNetwork.nextRequestId();
  }
  // If the inspector detached between the initial request and a redirect
  // hop, drop the requestId so we stop emitting half-events for it.
  if (inspectorRequestId !== null && inspectorNetwork === null) {
    inspectorRequestId = null;
  }
  if (inspectorRequestId !== null) {
    // hasPostData reflects whether the request carries a body at all,
    // including streamed bodies (reqRid !== null). The Rust capture uses
    // it to initialize `is_request_finished = !has_post_data`, so getting
    // this right is what keeps streaming-body requests un-finished until
    // a chunked `dataSent` path actually flushes them.
    const hasPostData = reqBody !== null || reqRid !== null;
    let postDataText;
    if (
      reqBody !== null &&
      TypedArrayPrototypeGetSymbolToStringTag(reqBody) === "Uint8Array"
    ) {
      try {
        postDataText = core.decode(reqBody);
      } catch {
        postDataText = undefined;
      }
    }
    const requestHeadersForCdp = joinHeaderValuesForCdp(
      req.headerList,
      /* lowerCaseNames */ true,
    );
    // The buffer's request_charset gates `Network.getRequestPostData`
    // (utf-8 only). Prefer an explicit charset from Content-Type; fall
    // back to utf-8 when we successfully decoded the body as a JS string,
    // since fetch encodes string bodies as utf-8 over the wire.
    let requestCharset = parseContentTypeForCdp(req.headerList).charset ||
      undefined;
    if (requestCharset === undefined && postDataText !== undefined) {
      requestCharset = "utf-8";
    }
    const requestWillBeSentParams = {
      requestId: inspectorRequestId,
      timestamp: DateNow() / 1000,
      wallTime: DateNow() / 1000,
      type: "Fetch",
      request: {
        url: req.currentUrl(),
        method: req.method,
        headers: requestHeadersForCdp,
        hasPostData,
        postData: postDataText,
      },
      // initiator: filled in by `op_inspector_emit_protocol_event` using
      // V8's current stack trace, so the user-code frame at the fetch()
      // call site is preserved.
      charset: requestCharset,
    };
    if (inspectorRedirectResponse !== null) {
      requestWillBeSentParams.redirectResponse = inspectorRedirectResponse;
    }
    safeEmit(inspectorNetwork.requestWillBeSent, requestWillBeSentParams);
    // When we supplied the entire request body inline via
    // `request.postData`, flip the buffer's `is_request_finished` flag
    // immediately so `Network.getRequestPostData` doesn't reject with
    // "Request data is not finished yet". Streaming bodies (reqRid !==
    // null, where postDataText stays undefined) take the chunked
    // `Network.dataSent` path instead and aren't covered here; keeping
    // them un-finished is the desired outcome until that lands.
    if (postDataText !== undefined) {
      safeEmit(inspectorNetwork.dataSent, {
        requestId: inspectorRequestId,
        finished: true,
      });
    }
  }

  function onAbort() {
    if (cancelHandleRid !== null) {
      core.tryClose(cancelHandleRid);
    }
  }
  terminator[abortSignal.add](onAbort);
  let resp;
  try {
    resp = await opFetchSend(requestRid);
  } catch (err) {
    if (inspectorRequestId !== null) {
      safeEmit(inspectorNetwork.loadingFailed, {
        requestId: inspectorRequestId,
        timestamp: DateNow() / 1000,
        type: "Fetch",
        errorText: err && err.message ? String(err.message) : String(err),
      });
    }
    if (terminator.aborted) return abortedNetworkError();
    throw err;
  } finally {
    if (cancelHandleRid !== null) {
      core.tryClose(cancelHandleRid);
    }
  }
  // Re-throw any body errors
  if (resp.error !== null) {
    if (inspectorRequestId !== null) {
      safeEmit(inspectorNetwork.loadingFailed, {
        requestId: inspectorRequestId,
        timestamp: DateNow() / 1000,
        type: "Fetch",
        errorText: resp.error[0],
      });
    }
    const { 0: message, 1: cause } = resp.error;
    throw new TypeError(message, { cause: new Error(cause) });
  }
  if (terminator.aborted) {
    // op_fetch_send resolved successfully, so the FetchResponseResource is already in
    // the resource table. The success path below either closes resp.responseRid
    // (redirect / null-body / HEAD / CONNECT) or hands it to createResponseBodyStream,
    // which owns its lifecycle. Only this aborted-after-resolve branch needs to close
    // the rid manually, otherwise it leaks and trips the test sanitizer.
    core.tryClose(resp.responseRid);
    return abortedNetworkError();
  }

  processUrlList(req.urlList, req.urlListProcessed);

  /** @type {InnerResponse} */
  const response = {
    headerList: resp.headers,
    status: resp.status,
    body: null,
    bodyDecoded: resp.bodyDecoded,
    statusMessage: resp.statusText,
    type: "basic",
    url() {
      if (this.urlList.length == 0) return null;
      return this.urlList[this.urlList.length - 1];
    },
    urlList: req.urlListProcessed,
  };

  // ---- Inspector: Network.responseReceived -------------------------------
  // Skip when we're about to follow a redirect: the 30x response becomes
  // the `redirectResponse` on the next hop's `requestWillBeSent` instead
  // of its own `responseReceived`, matching Chrome DevTools' contract.
  const willFollowRedirect = redirectStatus(resp.status) &&
    req.redirectMode === "follow";
  let cdpResponse = null;
  if (inspectorRequestId !== null) {
    const { mimeType, charset } = parseContentTypeForCdp(resp.headers);
    const responseHeadersForCdp = joinHeaderValuesForCdp(
      resp.headers,
      /* lowerCaseNames */ false,
    );
    cdpResponse = {
      url: resp.url || req.currentUrl(),
      status: resp.status,
      statusText: resp.statusText,
      headers: responseHeadersForCdp,
      mimeType,
      charset,
    };
    if (!willFollowRedirect) {
      safeEmit(inspectorNetwork.responseReceived, {
        requestId: inspectorRequestId,
        timestamp: DateNow() / 1000,
        type: "Fetch",
        response: cdpResponse,
      });
    }
  }

  if (redirectStatus(resp.status)) {
    switch (req.redirectMode) {
      case "error":
        core.close(resp.responseRid);
        if (inspectorRequestId !== null) {
          safeEmit(inspectorNetwork.loadingFailed, {
            requestId: inspectorRequestId,
            timestamp: DateNow() / 1000,
            type: "Fetch",
            errorText:
              "Encountered redirect while redirect mode is set to 'error'",
          });
        }
        return networkError(
          "Encountered redirect while redirect mode is set to 'error'",
        );
      case "follow":
        core.close(resp.responseRid);
        return httpRedirectFetch(
          req,
          response,
          terminator,
          inspectorRequestId !== null
            ? { requestId: inspectorRequestId, redirectResponse: cdpResponse }
            : null,
        );
      case "manual":
        break;
    }
  }

  if (nullBodyStatus(response.status)) {
    core.close(resp.responseRid);
    if (inspectorRequestId !== null) {
      safeEmit(inspectorNetwork.loadingFinished, {
        requestId: inspectorRequestId,
        timestamp: DateNow() / 1000,
        encodedDataLength: 0,
      });
    }
  } else {
    if (req.method === "HEAD" || req.method === "CONNECT") {
      response.body = null;
      core.close(resp.responseRid);
      if (inspectorRequestId !== null) {
        safeEmit(inspectorNetwork.loadingFinished, {
          requestId: inspectorRequestId,
          timestamp: DateNow() / 1000,
          encodedDataLength: 0,
        });
      }
    } else {
      let bodyStream = createResponseBodyStream(resp.responseRid, terminator);
      // Tee the response body so the inspector can drain a copy in the
      // background for `Network.dataReceived` + `loadingFinished` /
      // `getResponseBody`, while the user still consumes the original.
      if (inspectorRequestId !== null) {
        try {
          const tee = bodyStream.tee();
          bodyStream = tee[0];
          drainResponseForInspector(
            tee[1],
            inspectorRequestId,
            inspectorNetwork,
          );
        } catch {
          // tee failed; leave bodyStream untouched, no inspector data
        }
      }
      response.body = new InnerBody(bodyStream);
    }
  }

  if (recursive) return response;

  if (response.urlList.length === 0) {
    processUrlList(req.urlList, req.urlListProcessed);
    response.urlList = [...new SafeArrayIterator(req.urlListProcessed)];
  }

  return response;
}

/**
 * @param {InnerRequest} request
 * @param {InnerResponse} response
 * @param {AbortSignal} terminator
 * @param {?{requestId: string, redirectResponse: object}} inspectorCtx
 *   When present, carries the inspector requestId across the redirect so
 *   the next hop's `requestWillBeSent` keeps the same id and gets the
 *   previous response attached as `redirectResponse`.
 * @returns {Promise<InnerResponse>}
 */
function httpRedirectFetch(request, response, terminator, inspectorCtx = null) {
  const locationHeaders = ArrayPrototypeFilter(
    response.headerList,
    (entry) => byteLowerCase(entry[0]) === "location",
  );
  if (locationHeaders.length === 0) {
    return response;
  }

  const currentURL = new URL(request.currentUrl());
  const locationURL = new URL(
    locationHeaders[0][1],
    response.url() ?? undefined,
  );
  if (locationURL.hash === "") {
    locationURL.hash = currentURL.hash;
  }
  if (locationURL.protocol !== "https:" && locationURL.protocol !== "http:") {
    return networkError("Can not redirect to a non HTTP(s) url");
  }
  if (request.redirectCount === 20) {
    return networkError("Maximum number of redirects (20) reached");
  }
  request.redirectCount++;
  if (
    response.status !== 303 &&
    request.body !== null &&
    request.body.source === null
  ) {
    return networkError(
      "Can not redeliver a streaming request body after a redirect",
    );
  }
  if (
    ((response.status === 301 || response.status === 302) &&
      request.method === "POST") ||
    (response.status === 303 &&
      request.method !== "GET" &&
      request.method !== "HEAD")
  ) {
    request.method = "GET";
    request.body = null;
    for (let i = 0; i < request.headerList.length; i++) {
      if (
        ArrayPrototypeIncludes(
          REQUEST_BODY_HEADER_NAMES,
          byteLowerCase(request.headerList[i][0]),
        )
      ) {
        ArrayPrototypeSplice(request.headerList, i, 1);
        i--;
      }
    }
  }

  // Drop confidential headers when redirecting to a less secure protocol
  // or to a different domain that is not a superdomain
  if (
    (locationURL.protocol !== currentURL.protocol &&
      locationURL.protocol !== "https:") ||
    (locationURL.host !== currentURL.host &&
      !isSubdomain(locationURL.host, currentURL.host))
  ) {
    for (let i = 0; i < request.headerList.length; i++) {
      if (
        ArrayPrototypeIncludes(
          REDIRECT_SENSITIVE_HEADER_NAMES,
          byteLowerCase(request.headerList[i][0]),
        )
      ) {
        ArrayPrototypeSplice(request.headerList, i, 1);
        i--;
      }
    }
  }

  if (request.body !== null) {
    const res = extractBody(request.body.source);
    request.body = res.body;
  }
  ArrayPrototypePush(request.urlList, () => locationURL.href);
  return mainFetch(request, true, terminator, inspectorCtx);
}

/**
 * @param {RequestInfo} input
 * @param {RequestInit} init
 */
function fetch(input, init = undefined) {
  let span;
  let snapshot;
  try {
    if (__telemetry.TRACING_ENABLED) {
      span = builtinTracer().startSpan("fetch", { kind: 2 });
      snapshot = enterSpan(span);
    }

    // There is an async dispatch later that causes a stack trace disconnect.
    // We reconnect it by assigning the result of that dispatch to `opPromise`,
    // awaiting `opPromise` in an inner function also named `fetch()` and
    // returning the result from that.
    let opPromise = undefined;
    // 1.
    const result = new Promise((resolve, reject) => {
      const prefix = "Failed to execute 'fetch'";
      webidl.requiredArguments(arguments.length, 1, prefix);
      // 2.
      const requestObject = new Request(input, init);

      if (span) {
        const context = ContextManager.active();
        for (
          const propagator of new SafeArrayIterator(__telemetry.PROPAGATORS)
        ) {
          propagator.inject(context, requestObject.headers, {
            set(carrier, key, value) {
              carrier.append(key, value);
            },
          });
        }

        updateSpanFromRequest(span, requestObject);
      }

      // 3.
      const request = toInnerRequest(requestObject);
      // 4.
      if (requestObject.signal.aborted) {
        if (span) {
          // Handles this case here as this is the only case where `result` promise
          // is settled immediately.
          updateSpanFromError(span, requestObject.signal.reason);
        }
        reject(abortFetch(request, null, requestObject.signal.reason));
        return;
      }
      // 7.
      let responseObject = null;
      // 9.
      let locallyAborted = false;
      // 10.
      function onabort() {
        locallyAborted = true;
        reject(
          abortFetch(request, responseObject, requestObject.signal.reason),
        );
      }
      requestObject.signal[abortSignal.add](onabort);

      if (!requestObject.headers.has("Accept")) {
        ArrayPrototypePush(request.headerList, ["Accept", "*/*"]);
      }

      if (!requestObject.headers.has("Accept-Language")) {
        ArrayPrototypePush(request.headerList, ["Accept-Language", "*"]);
      }

      // 12.
      opPromise = PromisePrototypeCatch(
        PromisePrototypeThen(
          mainFetch(request, false, requestObject.signal),
          (response) => {
            // 12.1.
            if (locallyAborted) return;
            // 12.2.
            if (response.aborted) {
              reject(
                abortFetch(
                  request,
                  responseObject,
                  requestObject.signal.reason,
                ),
              );
              requestObject.signal[abortSignal.remove](onabort);
              return;
            }
            // 12.3.
            if (response.type === "error") {
              const err = new TypeError(
                "Fetch failed: " + (response.error ?? "unknown error"),
              );
              reject(err);
              requestObject.signal[abortSignal.remove](onabort);
              return;
            }
            responseObject = fromInnerResponse(response, "immutable");

            if (span) {
              updateSpanFromClientResponse(span, responseObject);
            }

            resolve(responseObject);
            requestObject.signal[abortSignal.remove](onabort);
          },
        ),
        (err) => {
          reject(err);
          requestObject.signal[abortSignal.remove](onabort);
        },
      );
    });

    if (opPromise) {
      PromisePrototypeCatch(result, (e) => {
        if (span) {
          updateSpanFromError(span, e);
        }
      });
      return (async function fetch() {
      try {
        await opPromise;
        return result;
      } finally {
        span?.end();
      }
      })();
    }
    // We need to end the span when the promise settles.
    // WPT has a test that aborted fetch is settled in the same tick.
    // This means we cannot wrap the promise if it is already settled.
    // But this is OK, because we can just immediately end the span
    // in that case.
    if (span) {
      // XXX: This should always be true, otherwise `opPromise` would be present.
      if (op_fetch_promise_is_settled(result)) {
        // It's already settled.
        span?.end();
      } else {
        // Not settled yet, we can return a new wrapper promise.
        return SafePromisePrototypeFinally(result, () => {
          span?.end();
        });
      }
    }
    return result;
  } finally {
    if (snapshot) restoreSnapshot(snapshot);
  }
}

function abortFetch(request, responseObject, error) {
  if (request.body !== null) {
    // Cancel the body if we haven't taken it as a resource yet
    if (!request.body.streamOrStatic.locked) {
      request.body.cancel(error);
    }
  }
  if (responseObject !== null) {
    const response = toInnerResponse(responseObject);
    if (response.body !== null) response.body.error(error);
  }
  return error;
}

/**
 * Checks if the given string is a subdomain of the given domain.
 *
 * @param {String} subdomain
 * @param {String} domain
 * @returns {Boolean}
 */
function isSubdomain(subdomain, domain) {
  const dot = subdomain.length - domain.length - 1;
  return (
    dot > 0 &&
    subdomain[dot] === "." &&
    StringPrototypeEndsWith(subdomain, domain)
  );
}

/**
 * Handle the Response argument to the WebAssembly streaming APIs, after
 * resolving if it was passed as a promise. This function should be registered
 * through `Deno.core.setWasmStreamingCallback`.
 *
 * @param {any} source The source parameter that the WebAssembly streaming API
 * was called with. If it was called with a Promise, `source` is the resolved
 * value of that promise.
 * @param {number} rid An rid that represents the wasm streaming resource.
 */
function handleWasmStreaming(source, rid) {
  // This implements part of
  // https://webassembly.github.io/spec/web-api/#compile-a-potential-webassembly-response
  try {
    const res = webidl.converters["Response"](
      source,
      "Failed to execute 'WebAssembly.compileStreaming'",
      "Argument 1",
    );

    // 2.3.
    // The spec is ambiguous here, see
    // https://github.com/WebAssembly/spec/issues/1138. The WPT tests expect
    // the raw value of the Content-Type attribute lowercased. We ignore this
    // for file:// because file fetches don't have a Content-Type.
    if (!StringPrototypeStartsWith(res.url, "file://")) {
      const contentType = res.headers.get("Content-Type");
      if (
        typeof contentType !== "string" ||
        StringPrototypeToLowerCase(contentType) !== "application/wasm"
      ) {
        throw new TypeError("Invalid WebAssembly content type");
      }
    }

    // 2.5.
    if (!res.ok) {
      throw new TypeError(
        `Failed to receive WebAssembly content: HTTP status code ${res.status}`,
      );
    }

    // Pass the resolved URL to v8.
    op_wasm_streaming_set_url(rid, res.url);

    if (res.body !== null) {
      // 2.6.
      // Rather than consuming the body as an ArrayBuffer, this feeds each chunk
      // to the streaming compiler as soon as it's available. Instead of reading
      // the body chunk-by-chunk in JS and calling `op_wasm_streaming_feed` once
      // per chunk, hand the underlying stream resource to Rust and let a single
      // async op pump the bytes straight into V8's streaming compiler.
      const stream = res.body;
      const resourceBacking = getReadableStreamResourceBacking(stream);
      let streamRid, closeStreamRid;
      if (resourceBacking) {
        // Fast path: feed straight from the body's backing resource. Acquire a
        // reader and mark the stream disturbed (as the response-body fast path
        // does) so nothing else consumes it and, crucially, so the stream stays
        // referenced until we're done. Otherwise it could be GC'd mid-feed and
        // its finalizer would close (and thereby cancel the read of) the backing
        // resource out from under us.
        acquireReadableStreamDefaultReader(stream);
        readableStreamDisturb(stream);
        streamRid = resourceBacking.rid;
        // Only close the resource if the stream owns it (mirrors the fast path
        // in `readableStreamCollectIntoUint8Array`).
        closeStreamRid = resourceBacking.autoClose;
      } else {
        // We allocated the resource, so we own it and must close it.
        streamRid = resourceForReadableStream(stream);
        closeStreamRid = true;
      }

      PromisePrototypeThen(
        op_wasm_streaming_stream_feed(rid, streamRid),
        // 2.7
        () => {
          if (resourceBacking) readableStreamClose(stream);
          if (closeStreamRid) core.tryClose(streamRid);
          core.close(rid);
        },
        // 2.8
        (err) => {
          if (resourceBacking) readableStreamClose(stream);
          if (closeStreamRid) core.tryClose(streamRid);
          core.abortWasmStreaming(rid, err);
        },
      );
    } else {
      // 2.7
      core.close(rid);
    }
  } catch (err) {
    // 2.8
    core.abortWasmStreaming(rid, err);
  }
}

return { fetch, handleWasmStreaming, mainFetch };
})();

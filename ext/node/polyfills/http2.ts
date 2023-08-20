// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

const core = globalThis.Deno.core;
import { notImplemented, warnNotImplemented } from "ext:deno_node/_utils.ts";
import { EventEmitter } from "node:events";
import { Buffer } from "node:buffer";
import { Server, Socket, TCP } from "node:net";
import { TypedArray } from "ext:deno_node/internal/util/types.ts";
import {
  kMaybeDestroy,
  kUpdateTimer,
  setStreamTimeout,
} from "ext:deno_node/internal/stream_base_commons.ts";
import { FileHandle } from "node:fs/promises";
import { kStreamBaseField } from "ext:deno_node/internal_binding/stream_wrap.ts";
import { addTrailers, serveHttpOnConnection } from "ext:deno_http/00_serve.js";
import { type Deferred, deferred } from "ext:deno_node/_util/async.ts";
import { nextTick } from "ext:deno_node/_next_tick.ts";
import { TextEncoder } from "ext:deno_web/08_text_encoding.js";
import { Duplex } from "node:stream";
import { setImmediate } from "node:timers";
import {
  AbortError,
  ERR_HTTP2_CONNECT_AUTHORITY,
  ERR_HTTP2_CONNECT_PATH,
  ERR_HTTP2_CONNECT_SCHEME,
  ERR_HTTP2_GOAWAY_SESSION,
  ERR_HTTP2_HEADER_SINGLE_VALUE,
  ERR_HTTP2_INVALID_CONNECTION_HEADERS,
  ERR_HTTP2_INVALID_PSEUDOHEADER,
  ERR_HTTP2_INVALID_SESSION,
  ERR_HTTP2_INVALID_STREAM,
  ERR_HTTP2_SESSION_ERROR,
  ERR_HTTP2_STREAM_CANCEL,
  ERR_HTTP2_STREAM_ERROR,
  ERR_HTTP2_TRAILERS_ALREADY_SENT,
  ERR_HTTP2_TRAILERS_NOT_READY,
  ERR_INVALID_HTTP_TOKEN,
} from "ext:deno_node/internal/errors.ts";
import { _checkIsHttpToken } from "ext:deno_node/_http_common.ts";

const kSession = Symbol("session");
const kAlpnProtocol = Symbol("alpnProtocol");
const kAuthority = Symbol("authority");
const kEncrypted = Symbol("encrypted");
const kID = Symbol("id");
const kInit = Symbol("init");
const kInfoHeaders = Symbol("sent-info-headers");
const kLocalSettings = Symbol("local-settings");
const kNativeFields = Symbol("kNativeFields");
const kOptions = Symbol("options");
const kOrigin = Symbol("origin");
const kPendingRequestCalls = Symbol("kPendingRequestCalls");
const kProceed = Symbol("proceed");
const kProtocol = Symbol("protocol");
const kRemoteSettings = Symbol("remote-settings");
const kSelectPadding = Symbol("select-padding");
const kSentHeaders = Symbol("sent-headers");
const kSentTrailers = Symbol("sent-trailers");
const kServer = Symbol("server");
const kState = Symbol("state");
const kType = Symbol("type");
const kWriteGeneric = Symbol("write-generic");
const kTimeout = Symbol("timeout");
const kSensitiveHeaders = Symbol("nodejs.http2.sensitiveHeaders");

const STREAM_FLAGS_PENDING = 0x0;
const STREAM_FLAGS_READY = 0x1;
const STREAM_FLAGS_CLOSED = 0x2;
const STREAM_FLAGS_HEADERS_SENT = 0x4;
const STREAM_FLAGS_HEAD_REQUEST = 0x8;
const STREAM_FLAGS_ABORTED = 0x10;
const STREAM_FLAGS_HAS_TRAILERS = 0x20;

const SESSION_FLAGS_PENDING = 0x0;
const SESSION_FLAGS_READY = 0x1;
const SESSION_FLAGS_CLOSED = 0x2;
const SESSION_FLAGS_DESTROYED = 0x4;

const ENCODER = new TextEncoder();
type Http2Headers = Record<string, string | string[]>;

let debugEnabled = true;
function debug(...args) {
  if (debugEnabled) {
    console.log(...args);
  }
}

export class Http2Session extends EventEmitter {
  constructor(type, options /* socket */) {
    super();

    // TODO(bartlomieju): Handle sockets here

    this[kState] = {
      destroyCode: constants.NGHTTP2_NO_ERROR,
      flags: SESSION_FLAGS_PENDING,
      goawayCode: null,
      goawayLastStreamID: null,
      streams: new Map(),
      pendingStreams: new Set(),
      pendingAck: 0,
      writeQueueSize: 0,
      originSet: undefined,
    };

    this[kEncrypted] = undefined;
    this[kAlpnProtocol] = undefined;
    this[kType] = type;
    this[kTimeout] = null;
    // this[kProxySocket] = null;
    // this[kSocket] = socket;
    // this[kHandle] = undefined;

    // TODO(bartlomieju): connecting via socket
  }

  get encrypted(): boolean {
    return this[kEncrypted];
  }

  get alpnProtocol(): string | undefined {
    return this[kAlpnProtocol];
  }

  get originSet(): string[] | undefined {
    if (!this.encrypted || this.destroyed) {
      return undefined;
    }
    // TODO(bartlomieju):
    return [];
  }

  get connecting(): boolean {
    return (this[kState].flags & SESSION_FLAGS_READY) === 0;
  }

  get closed(): boolean {
    return !!(this[kState].flags & SESSION_FLAGS_CLOSED);
  }

  get destroyed(): boolean {
    return !!(this[kState].flags & SESSION_FLAGS_DESTROYED);
  }

  [kUpdateTimer]() {
    if (this.destroyed) {
      return;
    }
    if (this[kTimeout]) {
      this[kTimeout].refresh();
    }
  }

  setLocalWindowSize(_windowSize: number) {
    notImplemented("Http2Session.setLocalWindowSize");
  }

  ping(
    _payload: Buffer | TypedArray | DataView,
    _callback: () => void,
  ): boolean {
    notImplemented("Http2Session.ping");
    return false;
  }

  get socket(): Socket /*| TlsSocket*/ {
    warnNotImplemented("Http2Session.socket");
    return {};
  }

  get type(): number {
    return this[kType];
  }

  get pendingSettingsAck() {
    return this[kState].pendingAck > 0;
  }

  get state(): Record<string, unknown> {
    return {};
  }

  get localSettings(): Record<string, unknown> {
    notImplemented("Http2Session.localSettings");
    return {};
  }

  get remoteSettings(): Record<string, unknown> {
    notImplemented("Http2Session.remoteSettings");
    return {};
  }

  settings(_settings: Record<string, unknown>, _callback: () => void) {
    notImplemented("Http2Session.settings");
  }

  goaway(
    _code: number,
    _lastStreamID: number,
    _opaqueData: Buffer | TypedArray | DataView,
  ) {
    warnNotImplemented("Http2Session.goaway");
  }

  destroy(error = constants.NGHTTP2_NO_ERROR, code?: number) {
    if (this.destroyed) {
      return;
    }

    if (typeof error === "number") {
      code = error;
      error = code !== constants.NGHTTP2_NO_ERROR
        ? new ERR_HTTP2_SESSION_ERROR(code)
        : undefined;
    }
    if (code === undefined && error != null) {
      code = constants.NGHTTP2_INTERNAL_ERROR;
    }

    closeSession(this, code, error);
  }

  close(callback?: () => void) {
    if (this.closed || this.destroyed) {
      return;
    }

    this[kState].flags |= SESSION_FLAGS_CLOSED;
    if (typeof callback === "function") {
      this.once("close", callback);
    }
    this.goaway();
    this[kMaybeDestroy]();
  }

  [kMaybeDestroy](error?: number) {
    if (!error) {
      const state = this[kState];
      // Don't destroy if the session is not closed or there are pending or open
      // streams.
      if (
        !this.closed || state.streams.size > 0 || state.pendingStreams.size >
          0
      ) {
        return;
      }
    }
    this.destroy(error);
  }

  ref() {
    warnNotImplemented("Http2Session.ref");
  }

  unref() {
    warnNotImplemented("Http2Session.unref");
  }

  setTimeout(msecs: number, callback?: () => void) {
    setStreamTimeout(this, msecs, callback);
  }
}

function emitClose(session: Http2Session, error?: Error) {
  if (error) {
    session.emit("error", error);
  }
  session.emit("close");
}

function finishSessionClose(session: Http2Session, error?: Error) {
  // TODO(bartlomieju): handle sockets

  nextTick(emitClose, session, error);
}

function closeSession(session: Http2Session, code?: number, error?: Error) {
  const state = session[kState];
  state.flags |= SESSION_FLAGS_DESTROYED;
  state.destroyCode = code;

  session.setTimeout(0);
  session.removeAllListeners("timeout");

  // Destroy open and pending streams
  if (state.pendingStreams.size > 0 || state.streams.size > 0) {
    const cancel = new ERR_HTTP2_STREAM_CANCEL(error);
    state.pendingStreams.forEach((stream) => stream.destroy(cancel));
    state.streams.forEach((stream) => stream.destroy(cancel));
  }

  // TODO(bartlomieju): handle sockets
  debug(">>> closeSession", session._connRid, session._clientRid);
  core.tryClose(session._connRid);
  core.tryClose(session._clientRid);

  finishSessionClose(session, error);
}

export class ServerHttp2Session extends Http2Session {
  constructor() {
    super(constants.NGHTTP2_SESSION_SERVER, {});
  }

  altsvc(
    _alt: string,
    _originOrStream: number | string | URL | { origin: string },
  ) {
    notImplemented("ServerHttp2Session.altsvc");
  }

  origin(..._origins: (string | URL | { origin: string })[]) {
    notImplemented("ServerHttp2Session.origins");
  }
}

function assertValidPseudoHeader(header: string) {
  switch (header) {
    case ":authority":
    case ":path":
    case ":method":
    case ":scheme":
    case ":status":
      return;
    default:
      throw new ERR_HTTP2_INVALID_PSEUDOHEADER(header);
  }
}

export class ClientHttp2Session extends Http2Session {
  #connectPromise: Promise<void>;
  _clientRid: number;
  _connRid: number;

  constructor(
    authority: string | URL,
    options: Record<string, unknown>,
  ) {
    super(constants.NGHTTP2_SESSION_CLIENT, options);
    this[kPendingRequestCalls] = null;

    // TODO(bartlomieju): cleanup
    this.#connectPromise = (async () => {
      // debug(">>> before connect");
      const [clientRid, connRid] = await core.opAsync(
        "op_http2_connect",
        authority.toString(),
      );
      // debug(">>> after connect");
      this._clientRid = clientRid;
      this._connRid = connRid;
      // TODO(bartlomieju): save so it can be unrefed
      (async () => {
        try {
          await core.opAsync("op_http2_poll_client_connection", this._connRid);
        } catch (e) {
          console.error("Error in op_http2_poll_client_connection", e);
          this.emit("error", e);
        }
      })();
      this.emit("connect", this, {});
    })();
  }

  request(
    headers: Http2Headers,
    options?: Record<string, unknown>,
  ): ClientHttp2Stream {
    if (this.destroyed) {
      throw new ERR_HTTP2_INVALID_SESSION();
    }

    if (this.closed) {
      throw new ERR_HTTP2_GOAWAY_SESSION();
    }

    this[kUpdateTimer]();
    if (headers !== null && headers !== undefined) {
      const keys = Object.keys(headers);
      for (let i = 0; i < keys.length; i++) {
        const header = keys[i];
        if (header[0] === ":") {
          assertValidPseudoHeader(header);
        } else if (header && !_checkIsHttpToken(header)) {
          this.destroy(new ERR_INVALID_HTTP_TOKEN("Header name", header));
        }
      }
    }

    headers = Object.assign({ __proto__: null }, headers);
    options = { ...options };

    if (headers[constants.HTTP2_HEADER_METHOD] === undefined) {
      headers[constants.HTTP2_HEADER_METHOD] = constants.HTTP2_METHOD_GET;
    }

    const connect =
      headers[constants.HTTP2_HEADER_METHOD] === constants.HTTP2_METHOD_CONNECT;

    if (!connect || headers[constants.HTTP2_HEADER_PROTOCOL] !== undefined) {
      if (getAuthority(headers) === undefined) {
        headers[constants.HTTP2_HEADER_AUTHORITY] = this[kAuthority];
      }
      if (headers[constants.HTTP2_HEADER_SCHEME] === undefined) {
        headers[constants.HTTP2_HEADER_SCHEME] = this[kProtocol].slice(0, -1);
      }
      if (headers[constants.HTTP2_HEADER_PATH] === undefined) {
        headers[constants.HTTP2_HEADER_PATH] = "/";
      }
    } else {
      if (headers[constants.HTTP2_HEADER_AUTHORITY] === undefined) {
        throw new ERR_HTTP2_CONNECT_AUTHORITY();
      }
      if (headers[constants.HTTP2_HEADER_SCHEME] === undefined) {
        throw new ERR_HTTP2_CONNECT_SCHEME();
      }
      if (headers[constants.HTTP2_HEADER_PATH] === undefined) {
        throw new ERR_HTTP2_CONNECT_PATH();
      }
    }

    if (options.endStream === undefined) {
      const method = headers[constants.HTTP2_HEADER_METHOD];
      options.endStream = method === constants.HTTP2_METHOD_DELETE ||
        method === constants.HTTP2_METHOD_GET ||
        method === constants.HTTP2_METHOD_HEAD;
    } else {
      options.endStream = !!options.endStream;
    }

    const stream = new ClientHttp2Stream(
      options,
      this,
      this.#connectPromise,
      headers,
    );
    stream[kSentHeaders] = headers;
    stream[kOrigin] = `${headers[constants.HTTP2_HEADER_SCHEME]}://${
      getAuthority(headers)
    }`;

    if (options.endStream) {
      stream.end();
    }

    if (options.waitForTrailers) {
      stream[kState].flags |= STREAM_FLAGS_HAS_TRAILERS;
    }

    const { signal } = options;
    if (signal) {
      const aborter = () => {
        stream.destroy(new AbortError(undefined, { cause: signal.reason }));
      };
      if (signal.aborted) {
        aborter();
      } else {
        // TODO(bartlomieju): handle this
        // const disposable = EventEmitter.addAbortListener(signal, aborter);
        // stream.once("close", disposable[Symbol.dispose]);
      }
    }

    // TODO(bartlomieju): handle this
    // const onConnect = requestOnConnect.bind(stream, headerList, options);
    const onConnect = () => {};
    if (this.connecting) {
      if (this[kPendingRequestCalls] !== null) {
        this[kPendingRequestCalls].push(onConnect);
      } else {
        this[kPendingRequestCalls] = [onConnect];
        this.once("connect", () => {
          this[kPendingRequestCalls].forEach((f) => f());
          this[kPendingRequestCalls] = null;
        });
      }
    } else {
      onConnect();
    }

    return stream;
  }
}

function getAuthority(headers) {
  if (headers[constants.HTTP2_HEADER_AUTHORITY] !== undefined) {
    return headers[constants.HTTP2_HEADER_AUTHORITY];
  }
  if (headers[constants.HTTP2_HEADER_HOST] !== undefined) {
    return headers[constants.HTTP2_HEADER_HOST];
  }
  return undefined;
}

export class Http2Stream extends EventEmitter {
  #session: Http2Session;
  #headers: Deferred<Http2Headers>;
  #controllerPromise: Deferred<ReadableStreamDefaultController<Uint8Array>>;
  #readerPromise: Deferred<ReadableStream<Uint8Array>>;
  #closed: boolean;
  _response: Response;

  constructor(
    session: Http2Session,
    headers: Promise<Http2Headers>,
    controllerPromise: Promise<ReadableStreamDefaultController<Uint8Array>>,
    readerPromise: Promise<ReadableStream<Uint8Array>>,
  ) {
    super();
    this.#session = session;
    this.#headers = headers;
    this.#controllerPromise = controllerPromise;
    this.#readerPromise = readerPromise;
    this.#closed = false;
    nextTick(() => {
      (async () => {
        const headers = await this.#headers;
        this.emit("headers", headers);
      })();
      (async () => {
        const reader = await this.#readerPromise;
        if (reader) {
          for await (const data of reader) {
            this.emit("data", new Buffer(data));
          }
        }
        this.emit("end");
      })();
    });
  }

  // TODO(mmastrac): Implement duplex
  end() {
    (async () => {
      const controller = await this.#controllerPromise;
      controller.close();
    })();
  }

  write(buffer, callback?: () => void) {
    (async () => {
      const controller = await this.#controllerPromise;
      if (typeof buffer === "string") {
        controller.enqueue(ENCODER.encode(buffer));
      } else {
        controller.enqueue(buffer);
      }
      callback?.();
    })();
  }

  setEncoding(_encoding) {}

  resume() {
  }

  pause() {
  }

  get aborted(): boolean {
    notImplemented("Http2Stream.aborted");
    return false;
  }

  get bufferSize(): number {
    notImplemented("Http2Stream.bufferSize");
    return 0;
  }

  close(_code: number, _callback: () => void) {
    this.#closed = true;
    this.emit("close");
  }

  get closed(): boolean {
    return this.#closed;
  }

  get destroyed(): boolean {
    return false;
  }

  get endAfterHeaders(): boolean {
    notImplemented("Http2Stream.endAfterHeaders");
    return false;
  }

  get id(): number | undefined {
    notImplemented("Http2Stream.id");
    return undefined;
  }

  get pending(): boolean {
    notImplemented("Http2Stream.pending");
    return false;
  }

  priority(_options: Record<string, unknown>) {
    notImplemented("Http2Stream.priority");
  }

  get rstCode(): number {
    // notImplemented("Http2Stream.rstCode");
    return 0;
  }

  get sentHeaders(): boolean {
    notImplemented("Http2Stream.sentHeaders");
    return false;
  }

  get sentInfoHeaders(): Record<string, unknown> {
    notImplemented("Http2Stream.sentInfoHeaders");
    return {};
  }

  get sentTrailers(): Record<string, unknown> {
    notImplemented("Http2Stream.sentTrailers");
    return {};
  }

  get session(): Http2Session {
    return this.#session;
  }

  setTimeout(msecs: number, callback?: () => void) {
    setStreamTimeout(this, msecs, callback);
  }

  get state(): Record<string, unknown> {
    notImplemented("Http2Stream.state");
    return {};
  }

  sendTrailers(_headers: Record<string, unknown>) {
    addTrailers(this._response, [["grpc-status", "0"], ["grpc-message", "OK"]]);
  }
}

async function clientHttp2Request(
  session,
  sessionConnectPromise,
  headers,
  options,
) {
  debug(
    "waiting for connect promise",
    sessionConnectPromise,
    headers,
    options,
  );
  await sessionConnectPromise;

  const reqHeaders: string[][] = [];
  const pseudoHeaders = {};

  for (const [key, value] of Object.entries(headers)) {
    if (key[0] === ":") {
      pseudoHeaders[key] = value;
    } else {
      reqHeaders.push([key, Array.isArray(value) ? value[0] : value]);
    }
  }
  debug(
    "waited for connect promise",
    !!options.waitForTrailers,
    pseudoHeaders,
    reqHeaders,
  );

  return await core.opAsync(
    "op_http2_client_request",
    session._clientRid,
    pseudoHeaders,
    reqHeaders,
  );
}

export class ClientHttp2Stream extends Duplex {
  #requestPromise: Promise<[number, number]>;
  #responsePromise: Promise<void>;
  #rid: number | undefined = undefined;
  #bodyRid = 0;
  #waitForTrailers = false;
  #hasTrailersToRead = false;
  #encoding = "utf8";

  constructor(
    options: Record<string, unknown>,
    session: Http2Session,
    sessionConnectPromise: Promise<void>,
    headers: Record<string, string>,
  ) {
    options.allowHalfOpen = true;
    options.decodeString = false;
    options.autoDestroy = false;
    super(options);
    this.cork();
    this[kSession] = session;
    session[kState].pendingStreams.add(this);

    this._readableState.readingMore = true;

    this[kState] = {
      didRead: false,
      flags: STREAM_FLAGS_PENDING | STREAM_FLAGS_HEADERS_SENT,
      rstCode: constants.NGHTTP2_NO_ERROR,
      writeQueueSize: 0,
      trailersReady: false,
      endAfterHeaders: false,
      shutdownWritableCalled: false,
    };

    this.#requestPromise = clientHttp2Request(
      session,
      sessionConnectPromise,
      headers,
      options,
    );
    this.#waitForTrailers = !!options.waitForTrailers;
    debug(">>> created clienthttp2stream");
    this.#responsePromise = (async () => {
      debug(">>> before request promise", session._clientRid);
      const [streamRid, streamId] = await this.#requestPromise;
      this.#rid = streamRid;
      // TODO(bartlomieju): clean this up
      this.__rid = streamRid;
      this[kID] = streamId;
      debug(">>> after request promise", session._clientRid, this.#rid);
      const response = await core.opAsync(
        "op_http2_client_get_response",
        this.#rid,
      );
      debug(">>> after get response", response);
      this.#bodyRid = response.bodyRid;
      const headers = {
        ":status": response.statusCode,
        ...Object.fromEntries(response.headers),
      };
      debug(">>> emitting response", headers);
      this.emit("response", headers, 0);
      this.__response = response;
      // this.uncork();
      this.emit("ready");
    })();
  }

  [kUpdateTimer]() {
    if (this.destroyed) {
      return;
    }
    if (this[kTimeout]) {
      this[kTimeout].refresh();
    }
    if (this[kSession]) {
      this[kSession][kUpdateTimer]();
    }
  }

  [kInit](id) {
    const state = this[kState];
    state.flags |= STREAM_FLAGS_READY;

    const session = this[kSession];
    session[kState].pendingStreams.delete(this);
    session[kState].streams.set(id, this);

    // TODO(bartlomieju): handle socket handle

    this[kID] = id;
    this.uncork();
    this.emit("ready");
  }

  get bufferSize() {
    return this[kState].writeQueueSize + this.writableLength;
  }

  get endAfterHeaders() {
    return this[kState].endAfterHeaders;
  }

  get sentHeaders() {
    return this[kSentHeaders];
  }

  get sentTrailers() {
    return this[kSentTrailers];
  }

  get sendInfoHeaders() {
    return this[kInfoHeaders];
  }

  get pending(): boolean {
    return this[kID] === undefined;
  }

  get id(): number | undefined {
    return this[kID];
  }

  get session(): Http2Session {
    return this[kSession];
  }

  _onTimeout() {
    callTimeout(this, kSession);
  }

  get headersSent() {
    return !!(this[kState].flags & STREAM_FLAGS_HEADERS_SENT);
  }

  get aborted() {
    return !!(this[kState].flags & STREAM_FLAGS_ABORTED);
  }

  get headRequest() {
    return !!(this[kState].flags & STREAM_FLAGS_HEAD_REQUEST);
  }

  get rstCode() {
    return this[kState].rstCode;
  }

  get state(): Record<string, unknown> {
    notImplemented("Http2Stream.state");
    return {};
  }

  // [kAfterAsyncWrite]() {}

  // [kWriteGeneric]() {}

  // TODO:
  _write(chunk, encoding, callback?: () => void) {
    console.log(">>> _write", callback);
    if (typeof encoding === "function") {
      callback = encoding;
      encoding = "utf8";
    }
    let data;
    if (typeof encoding === "string") {
      data = ENCODER.encode(chunk);
    } else {
      data = chunk.buffer;
    }

    this.#requestPromise
      .then(() => {
        debug(">>> _write", this.#rid, data, encoding, callback);
        return core.opAsync(
          "op_http2_client_send_data",
          this.#rid,
          data,
        );
      })
      .then(() => callback?.())
      .catch((e) => callback?.(e));
  }

  // TODO:
  _writev(chunks, callback?: () => {}) {
    notImplemented("ClientHttp2Stream._writev");
  }

  _final(cb) {
    if (this.pending) {
      this.once("ready", () => this._final(cb));
      return;
    }

    shutdownWritable(this, cb);
  }

  // TODO:
  _read() {
    if (this.destroyed) {
      this.push(null);
      return;
    }

    if (!this[kState].didRead) {
      this._readableState.readingMore = false;
      this[kState].didRead = true;
    }
    // if (!this.pending) {
    //   streamOnResume(this);
    // } else {
    //   this.once("ready", () => streamOnResume(this));
    // }

    if (!this.__response) {
      this.once("ready", this._read);
      return;
    }

    debug(">>> read");
    // if (this.#hasTrailersToRead) {
    //   (async () => {
    //     debug(">>> before trailers to read");
    //     const trailerList = await core.opAsync(
    //       "op_http2_client_get_response_trailers",
    //       response.bodyRid,
    //     );
    //     debug(">>> after trailers to read");
    //     debug(">>> trailers", trailerList);
    //     const trailers = Object.fromEntries(trailerList);
    //     // core.close(response.bodyRid);
    //     // core.close(clientRid);
    //     // core.tryClose(connRid);
    //     this.emit("trailers", trailers);
    //   })();

    //   // this.#closed = true;
    //   // // debug(this.#bodyRid, this.#rid);
    //   // core.tryClose(this.#bodyRid);
    //   // core.tryClose(this.#rid);
    //   // this.emit("end");
    // }

    (async () => {
      const [chunk, finished] = await core.opAsync(
        "op_http2_client_get_response_body_chunk",
        this.__response.bodyRid,
      );

      debug(">>> chunk", chunk, finished);
      if (chunk === null) {
        // if (finished) {
        //   this.#hasTrailersToRead = true;
        //   const result = this.push(null);
        //   debug(">>> read result1", result);
        //   // TODO: remove, don't emit manually
        //   this.emit("end");
        //   return;
        // }
        // const trailerList = await core.opAsync(
        //   "op_http2_client_get_response_trailers",
        //   this.__response.bodyRid,
        // );
        // // debug(">>> after trailers to read");
        // // debug(">>> trailers", trailerList);
        // const trailers = Object.fromEntries(trailerList);
        // // core.close(response.bodyRid);
        // // core.close(clientRid);
        // // core.tryClose(connRid);
        // debug(">>> emitting trailers", trailers);
        // const result = this.push(null);
        // debug(">>> read result2", result);
        // this.emit("trailers", trailers);
        // // TODO: remove, don't emit manually
        // this.emit("end");
        this.push(null);
        onStreamTrailers(this);
        debug(">>> read null chunk");
        this[kMaybeDestroy]();
        return;
      }

      let result;
      if (this.#encoding === "utf8") {
        result = this.push(new TextDecoder().decode(new Uint8Array(chunk)));
      } else {
        result = this.push(new Uint8Array(chunk));
      }
      debug(">>> read result", result);
    })();
  }

  // TODO:
  priority(_options: Record<string, unknown>) {
    notImplemented("Http2Stream.priority");
  }

  sendTrailers(trailers: Record<string, unknown>) {
    console.log("sendTrailers", trailers);
    if (this.destroyed || this.closed) {
      throw new ERR_HTTP2_INVALID_STREAM();
    }
    if (this[kSentTrailers]) {
      throw new ERR_HTTP2_TRAILERS_ALREADY_SENT();
    }
    if (!this[kState].trailersReady) {
      throw new ERR_HTTP2_TRAILERS_NOT_READY();
    }

    trailers = Object.assign({ __proto__: null }, trailers);
    const trailerList = [];
    for (const [key, value] of Object.entries(trailers)) {
      trailerList.push([key, value]);
    }
    this[kSentTrailers] = trailers;

    const stream = this;
    const rid = this.#rid;
    setImmediate(() => {
      if (stream.destroyed) {
        return;
      }

      stream[kState].flags &= ~STREAM_FLAGS_HAS_TRAILERS;
      console.log("sending trailers", rid, trailers);

      core.opAsync(
        "op_http2_client_send_trailers",
        rid,
        trailerList,
      ).then(() => {
        stream[kMaybeDestroy]();
      }).catch((e) => {
        debug(">>> send trailers error", e);
        stream._destroy(new Error("boom!"));
      });
    });
  }

  get closed(): boolean {
    return !!(this[kState].flags & STREAM_FLAGS_CLOSED);
  }

  // TODO:
  close(code: number = constants.NGHTTP2_NO_ERROR, callback?: () => void) {
    console.log(">>> close", callback);
    console.error("Stream close");

    if (this.closed) {
      return;
    }
    if (typeof callback !== undefined) {
      this.once("close", callback);
    }
    closeStream(this, code);
  }

  _destroy(err, callback) {
    debug(">>> ClientHttp2Stream._destroy", err, callback);
    const session = this[kSession];
    const id = this[kID];

    const state = this[kState];
    const sessionState = session[kState];
    const sessionCode = sessionState.goawayCode || sessionState.destroyCode;

    let code = this.closed ? this.rstCode : sessionCode;
    if (err != null) {
      if (sessionCode) {
        code = sessionCode;
      } else if (err instanceof AbortError) {
        code = constants.NGHTTP2_CANCEL;
      } else {
        code = constants.NGHTTP2_INTERNAL_ERROR;
      }
    }

    if (!this.closed) {
      // TODO: this not handle `socket handle`
      closeStream(this, code, kNoRstStream);
    }

    sessionState.streams.delete(id);
    sessionState.pendingStreams.delete(this);

    sessionState.writeQueueSize -= state.writeQueueSize;
    state.writeQueueSize = 0;

    const nameForErrorCode = {};
    if (
      err == null && code !== constants.NGHTTP2_NO_ERROR &&
      code !== constants.NGHTTP2_CANCEL
    ) {
      err = new ERR_HTTP2_STREAM_ERROR(nameForErrorCode[code] || code);
    }

    this[kSession] = undefined;

    session[kMaybeDestroy]();
    callback(err);
  }

  [kMaybeDestroy](code = constants.NGHTTP2_NO_ERROR) {
    debug(
      ">>> ClientHttp2Stream[kMaybeDestroy]",
      code,
      this.writableFinished,
      this.readable,
      this.closed,
    );
    if (code !== constants.NGHTTP2_NO_ERROR) {
      this._destroy();
      return;
    }

    if (this.writableFinished) {
      if (!this.readable && this.closed) {
        console.log("going into _destroy");
        this._destroy();
        return;
      }

      const state = this[kState];
      console.log(
        "state here, sent headers",
        this.headersSent,
        "has trailers",
        !(state.flags & STREAM_FLAGS_HAS_TRAILERS),
        "didRead",
        state.didRead,
        "readableFlowing",
        this.readableFlowing,
      );
      if (
        this.headersSent && !(state.flags & STREAM_FLAGS_HAS_TRAILERS) &&
        !state.didRead && this.readableFlowing === null
      ) {
        setImmediate(() => {
          this.close();
        });
      }
    }
  }

  setTimeout(msecs: number, callback?: () => void) {
    // setStreamTimeout(this, msecs, callback);
    notImplemented("ClientHttp2Stream.setTimeout");
  }
}

function shutdownWritable(stream, callback) {
  console.log(">>> shutdownWritable", callback);
  const state = stream[kState];
  if (state.shutdownWritableCalled) {
    return callback();
  }
  state.shutdownWritableCalled = true;
  core.opAsync(
    "op_http2_client_end_stream",
    // TODO(bartlomieju): cleanup
    stream.__rid,
  ).then(() => {
    debug(">>> endstream close", stream.__rid);
    core.tryClose(stream.__rid);
    callback();
  });
  // TODO(bartlomieju): might have to add "finish" event listener here,
  // check it.
}

function onStreamTrailers(stream) {
  stream[kState].trailersReady = true;
  debug(">>> onStreamTrailers", stream.destroyed, stream.closed);
  if (stream.destroyed || stream.closed) {
    return;
  }
  if (!stream.emit("wantTrailers")) {
    debug(">>> onStreamTrailers no wantTrailers");
    stream.sendTrailers({});
  }
  debug(">>> onStreamTrailers wantTrailers");
}

const kNoRstStream = 0;
const kSubmitRstStream = 1;
const kForceRstStream = 2;

function closeStream(stream, code, rstStreamStatus = kSubmitRstStream) {
  const state = stream[kState];
  state.flags |= STREAM_FLAGS_CLOSED;
  state.rstCode = code;

  stream.setTimeout(0);
  stream.removeAllListeners("timeout");

  const { ending } = stream._writableState;

  if (!ending) {
    if (!stream.aborted) {
      state.flags |= STREAM_FLAGS_ABORTED;
      stream.emit("aborted");
    }

    stream.end();
  }

  if (rstStreamStatus != kNoRstStream) {
    if (
      !ending || stream.writableFinished ||
      code !== constants.NGHTTP2_NO_ERROR || rstStreamStatus === kForceRstStream
    ) {
      finishCloseStream(stream, code);
    } else {
      stream.once("finish", () => finishCloseStream(stream, code));
    }
  }
}

function finishCloseStream(stream, code) {
  debug(">>> finishCloseStream", stream, code);
  if (stream.pending) {
    this.push(null);
    this.once("ready", () => {
      core.opAsync(
        "op_http2_client_reset_stream",
        this.__rid,
        code,
      ).then(() => {
        debug(
          ">>> finishCloseStream close",
          this.__rid,
          this.__response.bodyRid,
        );
        core.tryClose(this.__rid);
        core.tryClose(this.__response.bodyRid);
      });
    });
  } else {
    core.opAsync(
      "op_http2_client_reset_stream",
      this.__rid,
      code,
    ).then(() => {
      debug(
        ">>> finishCloseStream close2",
        this.__rid,
        this.__response.bodyRid,
      );
      core.tryClose(this.__rid);
      core.tryClose(this.__response.bodyRid);
    });
  }
}

function callTimeout() {
  notImplemented("callTimeout");
}

export class ServerHttp2Stream extends Http2Stream {
  _promise: Deferred<Response>;
  #body: ReadableStream<Uint8Array>;
  #waitForTrailers: boolean;
  #headersSent: boolean;

  constructor(
    session: Http2Session,
    headers: Promise<Http2Headers>,
    controllerPromise: Promise<ReadableStreamDefaultController<Uint8Array>>,
    reader: ReadableStream<Uint8Array>,
    body: ReadableStream<Uint8Array>,
  ) {
    super(session, headers, controllerPromise, Promise.resolve(reader));
    this._promise = new deferred();
    this.#body = body;
  }

  additionalHeaders(_headers: Record<string, unknown>) {
    notImplemented("ServerHttp2Stream.additionalHeaders");
  }

  end(): void {
    super.end();
    if (this.#waitForTrailers) {
      this.emit("wantTrailers");
    }
  }

  get headersSent(): boolean {
    return this.#headersSent;
  }

  get pushAllowed(): boolean {
    notImplemented("ServerHttp2Stream.pushAllowed");
    return false;
  }

  pushStream(
    _headers: Record<string, unknown>,
    _options: Record<string, unknown>,
    _callback: () => unknown,
  ) {
    notImplemented("ServerHttp2Stream.pushStream");
  }

  respond(
    headers: Http2Headers,
    options: Record<string, unknown>,
  ) {
    this.#headersSent = true;
    const response: ResponseInit = {};
    if (headers) {
      for (const [name, value] of Object.entries(headers)) {
        if (name == constants.HTTP2_HEADER_STATUS) {
          response.status = Number(value);
        }
      }
    }
    if (options?.endStream) {
      this._promise.resolve(this._response = new Response("", response));
    } else {
      this.#waitForTrailers = options?.waitForTrailers;
      this._promise.resolve(
        this._response = new Response(this.#body, response),
      );
    }
  }

  respondWithFD(
    _fd: number | FileHandle,
    _headers: Record<string, unknown>,
    _options: Record<string, unknown>,
  ) {
    notImplemented("ServerHttp2Stream.respondWithFD");
  }

  respondWithFile(
    _path: string | Buffer | URL,
    _headers: Record<string, unknown>,
    _options: Record<string, unknown>,
  ) {
    notImplemented("ServerHttp2Stream.respondWithFile");
  }
}

export class Http2Server extends Server {
  #options: Record<string, unknown> = {};
  #abortController;
  #server;
  timeout = 0;

  constructor(
    options: Record<string, unknown>,
    requestListener: () => unknown,
  ) {
    super(options);
    this.#abortController = new AbortController();
    this.on(
      "connection",
      (conn: Deno.Conn) => {
        try {
          const session = new ServerHttp2Session();
          this.emit("session", session);
          this.#server = serveHttpOnConnection(
            conn,
            this.#abortController.signal,
            async (req: Request) => {
              try {
                const controllerPromise: Deferred<
                  ReadableStreamDefaultController<Uint8Array>
                > = deferred();
                const body = new ReadableStream({
                  start(controller) {
                    controllerPromise.resolve(controller);
                  },
                });
                const headers: Http2Headers = {};
                for (const [name, value] of req.headers) {
                  headers[name] = value;
                }
                headers[constants.HTTP2_HEADER_PATH] =
                  new URL(req.url).pathname;
                const stream = new ServerHttp2Stream(
                  session,
                  Promise.resolve(headers),
                  controllerPromise,
                  req.body,
                  body,
                );
                session.emit("stream", stream, headers);
                this.emit("stream", stream, headers);
                return await stream._promise;
              } catch (e) {
                console.log(">>> Error in serveHttpOnConnection", e);
              }
              return new Response("");
            },
            () => {
              console.log(">>> error");
            },
            () => {},
          );
        } catch (e) {
          console.log(">>> Error in Http2Server", e);
        }
      },
    );
    this.on(
      "newListener",
      (event) => console.log(`Event in newListener: ${event}`),
    );
    this.#options = options;
    if (typeof requestListener === "function") {
      this.on("request", requestListener);
    }
  }

  // Prevent the TCP server from wrapping this in a socket, since we need it to serve HTTP
  _createSocket(clientHandle: TCP) {
    return clientHandle[kStreamBaseField];
  }

  close(callback?: () => unknown) {
    if (callback) {
      this.on("close", callback);
    }
    this.#abortController.abort();
    super.close();
  }

  setTimeout(msecs: number, callback?: () => unknown) {
    this.timeout = msecs;
    if (callback !== undefined) {
      this.on("timeout", callback);
    }
  }

  updateSettings(settings: Record<string, unknown>) {
    this.#options.settings = { ...this.#options.settings, ...settings };
  }
}

export class Http2SecureServer extends Server {
  #options: Record<string, unknown> = {};
  timeout = 0;

  constructor(
    options: Record<string, unknown>,
    requestListener: () => unknown,
  ) {
    super(options, function () {
      notImplemented("connectionListener");
    });
    this.#options = options;
    if (typeof requestListener === "function") {
      this.on("request", requestListener);
    }
  }

  close(_callback?: () => unknown) {
    notImplemented("Http2SecureServer.close");
  }

  setTimeout(msecs: number, callback?: () => unknown) {
    this.timeout = msecs;
    if (callback !== undefined) {
      this.on("timeout", callback);
    }
  }

  updateSettings(settings: Record<string, unknown>) {
    this.#options.settings = { ...this.#options.settings, ...settings };
  }
}

export function createServer(
  options: Record<string, unknown>,
  onRequestHandler: () => unknown,
): Http2Server {
  if (typeof options === "function") {
    onRequestHandler = options;
    options = {};
  }
  return new Http2Server(options, onRequestHandler);
}

export function createSecureServer(
  _options: Record<string, unknown>,
  _onRequestHandler: () => unknown,
): Http2SecureServer {
  notImplemented("http2.createSecureServer");
  return new Http2SecureServer();
}

export function connect(
  authority: string | URL,
  options: Record<string, unknown>,
  callback: (session: ClientHttp2Session) => void,
): ClientHttp2Session {
  console.log(">>> http2.connect", options);

  if (typeof options === "function") {
    callback = options;
    options = undefined;
  }

  options = { ...options };

  if (typeof authority === "string") {
    authority = new URL(authority);
  }

  const protocol = authority.protocol || options.protocol || "https:";
  let port = "";

  if (authority.port !== "") {
    port += authority.port;
  } else if (protocol === "http:") {
    port += 80;
  } else {
    port += 443;
  }
  let host = "localhost";

  if (authority.hostname) {
    host = authority.hostname;

    if (host[0] === "[") {
      host = host.slice(1, -1);
    }
  } else if (authority.host) {
    host = authority.host;
  }

  // TODO: handle defaults
  if (typeof options.createConnection === "function") {
    console.error("Not implemented: http2.connect.options.createConnection");
    // notImplemented("http2.connect.options.createConnection");
  }

  const session = new ClientHttp2Session(authority, options);
  session[kAuthority] = `${options.servername || host}:${port}`;
  session[kProtocol] = protocol;

  if (typeof callback === "function") {
    session.once("connect", callback);
  }
  return session;
}

export const constants = {
  NGHTTP2_ERR_FRAME_SIZE_ERROR: -522,
  NGHTTP2_NV_FLAG_NONE: 0,
  NGHTTP2_NV_FLAG_NO_INDEX: 1,
  NGHTTP2_SESSION_SERVER: 0,
  NGHTTP2_SESSION_CLIENT: 1,
  NGHTTP2_STREAM_STATE_IDLE: 1,
  NGHTTP2_STREAM_STATE_OPEN: 2,
  NGHTTP2_STREAM_STATE_RESERVED_LOCAL: 3,
  NGHTTP2_STREAM_STATE_RESERVED_REMOTE: 4,
  NGHTTP2_STREAM_STATE_HALF_CLOSED_LOCAL: 5,
  NGHTTP2_STREAM_STATE_HALF_CLOSED_REMOTE: 6,
  NGHTTP2_STREAM_STATE_CLOSED: 7,
  NGHTTP2_FLAG_NONE: 0,
  NGHTTP2_FLAG_END_STREAM: 1,
  NGHTTP2_FLAG_END_HEADERS: 4,
  NGHTTP2_FLAG_ACK: 1,
  NGHTTP2_FLAG_PADDED: 8,
  NGHTTP2_FLAG_PRIORITY: 32,
  DEFAULT_SETTINGS_HEADER_TABLE_SIZE: 4096,
  DEFAULT_SETTINGS_ENABLE_PUSH: 1,
  DEFAULT_SETTINGS_MAX_CONCURRENT_STREAMS: 4294967295,
  DEFAULT_SETTINGS_INITIAL_WINDOW_SIZE: 65535,
  DEFAULT_SETTINGS_MAX_FRAME_SIZE: 16384,
  DEFAULT_SETTINGS_MAX_HEADER_LIST_SIZE: 65535,
  DEFAULT_SETTINGS_ENABLE_CONNECT_PROTOCOL: 0,
  MAX_MAX_FRAME_SIZE: 16777215,
  MIN_MAX_FRAME_SIZE: 16384,
  MAX_INITIAL_WINDOW_SIZE: 2147483647,
  NGHTTP2_SETTINGS_HEADER_TABLE_SIZE: 1,
  NGHTTP2_SETTINGS_ENABLE_PUSH: 2,
  NGHTTP2_SETTINGS_MAX_CONCURRENT_STREAMS: 3,
  NGHTTP2_SETTINGS_INITIAL_WINDOW_SIZE: 4,
  NGHTTP2_SETTINGS_MAX_FRAME_SIZE: 5,
  NGHTTP2_SETTINGS_MAX_HEADER_LIST_SIZE: 6,
  NGHTTP2_SETTINGS_ENABLE_CONNECT_PROTOCOL: 8,
  PADDING_STRATEGY_NONE: 0,
  PADDING_STRATEGY_ALIGNED: 1,
  PADDING_STRATEGY_MAX: 2,
  PADDING_STRATEGY_CALLBACK: 1,
  NGHTTP2_NO_ERROR: 0,
  NGHTTP2_PROTOCOL_ERROR: 1,
  NGHTTP2_INTERNAL_ERROR: 2,
  NGHTTP2_FLOW_CONTROL_ERROR: 3,
  NGHTTP2_SETTINGS_TIMEOUT: 4,
  NGHTTP2_STREAM_CLOSED: 5,
  NGHTTP2_FRAME_SIZE_ERROR: 6,
  NGHTTP2_REFUSED_STREAM: 7,
  NGHTTP2_CANCEL: 8,
  NGHTTP2_COMPRESSION_ERROR: 9,
  NGHTTP2_CONNECT_ERROR: 10,
  NGHTTP2_ENHANCE_YOUR_CALM: 11,
  NGHTTP2_INADEQUATE_SECURITY: 12,
  NGHTTP2_HTTP_1_1_REQUIRED: 13,
  NGHTTP2_DEFAULT_WEIGHT: 16,
  HTTP2_HEADER_STATUS: ":status",
  HTTP2_HEADER_METHOD: ":method",
  HTTP2_HEADER_AUTHORITY: ":authority",
  HTTP2_HEADER_SCHEME: ":scheme",
  HTTP2_HEADER_PATH: ":path",
  HTTP2_HEADER_PROTOCOL: ":protocol",
  HTTP2_HEADER_ACCEPT_ENCODING: "accept-encoding",
  HTTP2_HEADER_ACCEPT_LANGUAGE: "accept-language",
  HTTP2_HEADER_ACCEPT_RANGES: "accept-ranges",
  HTTP2_HEADER_ACCEPT: "accept",
  HTTP2_HEADER_ACCESS_CONTROL_ALLOW_CREDENTIALS:
    "access-control-allow-credentials",
  HTTP2_HEADER_ACCESS_CONTROL_ALLOW_HEADERS: "access-control-allow-headers",
  HTTP2_HEADER_ACCESS_CONTROL_ALLOW_METHODS: "access-control-allow-methods",
  HTTP2_HEADER_ACCESS_CONTROL_ALLOW_ORIGIN: "access-control-allow-origin",
  HTTP2_HEADER_ACCESS_CONTROL_EXPOSE_HEADERS: "access-control-expose-headers",
  HTTP2_HEADER_ACCESS_CONTROL_REQUEST_HEADERS: "access-control-request-headers",
  HTTP2_HEADER_ACCESS_CONTROL_REQUEST_METHOD: "access-control-request-method",
  HTTP2_HEADER_AGE: "age",
  HTTP2_HEADER_AUTHORIZATION: "authorization",
  HTTP2_HEADER_CACHE_CONTROL: "cache-control",
  HTTP2_HEADER_CONNECTION: "connection",
  HTTP2_HEADER_CONTENT_DISPOSITION: "content-disposition",
  HTTP2_HEADER_CONTENT_ENCODING: "content-encoding",
  HTTP2_HEADER_CONTENT_LENGTH: "content-length",
  HTTP2_HEADER_CONTENT_TYPE: "content-type",
  HTTP2_HEADER_COOKIE: "cookie",
  HTTP2_HEADER_DATE: "date",
  HTTP2_HEADER_ETAG: "etag",
  HTTP2_HEADER_FORWARDED: "forwarded",
  HTTP2_HEADER_HOST: "host",
  HTTP2_HEADER_IF_MODIFIED_SINCE: "if-modified-since",
  HTTP2_HEADER_IF_NONE_MATCH: "if-none-match",
  HTTP2_HEADER_IF_RANGE: "if-range",
  HTTP2_HEADER_LAST_MODIFIED: "last-modified",
  HTTP2_HEADER_LINK: "link",
  HTTP2_HEADER_LOCATION: "location",
  HTTP2_HEADER_RANGE: "range",
  HTTP2_HEADER_REFERER: "referer",
  HTTP2_HEADER_SERVER: "server",
  HTTP2_HEADER_SET_COOKIE: "set-cookie",
  HTTP2_HEADER_STRICT_TRANSPORT_SECURITY: "strict-transport-security",
  HTTP2_HEADER_TRANSFER_ENCODING: "transfer-encoding",
  HTTP2_HEADER_TE: "te",
  HTTP2_HEADER_UPGRADE_INSECURE_REQUESTS: "upgrade-insecure-requests",
  HTTP2_HEADER_UPGRADE: "upgrade",
  HTTP2_HEADER_USER_AGENT: "user-agent",
  HTTP2_HEADER_VARY: "vary",
  HTTP2_HEADER_X_CONTENT_TYPE_OPTIONS: "x-content-type-options",
  HTTP2_HEADER_X_FRAME_OPTIONS: "x-frame-options",
  HTTP2_HEADER_KEEP_ALIVE: "keep-alive",
  HTTP2_HEADER_PROXY_CONNECTION: "proxy-connection",
  HTTP2_HEADER_X_XSS_PROTECTION: "x-xss-protection",
  HTTP2_HEADER_ALT_SVC: "alt-svc",
  HTTP2_HEADER_CONTENT_SECURITY_POLICY: "content-security-policy",
  HTTP2_HEADER_EARLY_DATA: "early-data",
  HTTP2_HEADER_EXPECT_CT: "expect-ct",
  HTTP2_HEADER_ORIGIN: "origin",
  HTTP2_HEADER_PURPOSE: "purpose",
  HTTP2_HEADER_TIMING_ALLOW_ORIGIN: "timing-allow-origin",
  HTTP2_HEADER_X_FORWARDED_FOR: "x-forwarded-for",
  HTTP2_HEADER_PRIORITY: "priority",
  HTTP2_HEADER_ACCEPT_CHARSET: "accept-charset",
  HTTP2_HEADER_ACCESS_CONTROL_MAX_AGE: "access-control-max-age",
  HTTP2_HEADER_ALLOW: "allow",
  HTTP2_HEADER_CONTENT_LANGUAGE: "content-language",
  HTTP2_HEADER_CONTENT_LOCATION: "content-location",
  HTTP2_HEADER_CONTENT_MD5: "content-md5",
  HTTP2_HEADER_CONTENT_RANGE: "content-range",
  HTTP2_HEADER_DNT: "dnt",
  HTTP2_HEADER_EXPECT: "expect",
  HTTP2_HEADER_EXPIRES: "expires",
  HTTP2_HEADER_FROM: "from",
  HTTP2_HEADER_IF_MATCH: "if-match",
  HTTP2_HEADER_IF_UNMODIFIED_SINCE: "if-unmodified-since",
  HTTP2_HEADER_MAX_FORWARDS: "max-forwards",
  HTTP2_HEADER_PREFER: "prefer",
  HTTP2_HEADER_PROXY_AUTHENTICATE: "proxy-authenticate",
  HTTP2_HEADER_PROXY_AUTHORIZATION: "proxy-authorization",
  HTTP2_HEADER_REFRESH: "refresh",
  HTTP2_HEADER_RETRY_AFTER: "retry-after",
  HTTP2_HEADER_TRAILER: "trailer",
  HTTP2_HEADER_TK: "tk",
  HTTP2_HEADER_VIA: "via",
  HTTP2_HEADER_WARNING: "warning",
  HTTP2_HEADER_WWW_AUTHENTICATE: "www-authenticate",
  HTTP2_HEADER_HTTP2_SETTINGS: "http2-settings",
  HTTP2_METHOD_ACL: "ACL",
  HTTP2_METHOD_BASELINE_CONTROL: "BASELINE-CONTROL",
  HTTP2_METHOD_BIND: "BIND",
  HTTP2_METHOD_CHECKIN: "CHECKIN",
  HTTP2_METHOD_CHECKOUT: "CHECKOUT",
  HTTP2_METHOD_CONNECT: "CONNECT",
  HTTP2_METHOD_COPY: "COPY",
  HTTP2_METHOD_DELETE: "DELETE",
  HTTP2_METHOD_GET: "GET",
  HTTP2_METHOD_HEAD: "HEAD",
  HTTP2_METHOD_LABEL: "LABEL",
  HTTP2_METHOD_LINK: "LINK",
  HTTP2_METHOD_LOCK: "LOCK",
  HTTP2_METHOD_MERGE: "MERGE",
  HTTP2_METHOD_MKACTIVITY: "MKACTIVITY",
  HTTP2_METHOD_MKCALENDAR: "MKCALENDAR",
  HTTP2_METHOD_MKCOL: "MKCOL",
  HTTP2_METHOD_MKREDIRECTREF: "MKREDIRECTREF",
  HTTP2_METHOD_MKWORKSPACE: "MKWORKSPACE",
  HTTP2_METHOD_MOVE: "MOVE",
  HTTP2_METHOD_OPTIONS: "OPTIONS",
  HTTP2_METHOD_ORDERPATCH: "ORDERPATCH",
  HTTP2_METHOD_PATCH: "PATCH",
  HTTP2_METHOD_POST: "POST",
  HTTP2_METHOD_PRI: "PRI",
  HTTP2_METHOD_PROPFIND: "PROPFIND",
  HTTP2_METHOD_PROPPATCH: "PROPPATCH",
  HTTP2_METHOD_PUT: "PUT",
  HTTP2_METHOD_REBIND: "REBIND",
  HTTP2_METHOD_REPORT: "REPORT",
  HTTP2_METHOD_SEARCH: "SEARCH",
  HTTP2_METHOD_TRACE: "TRACE",
  HTTP2_METHOD_UNBIND: "UNBIND",
  HTTP2_METHOD_UNCHECKOUT: "UNCHECKOUT",
  HTTP2_METHOD_UNLINK: "UNLINK",
  HTTP2_METHOD_UNLOCK: "UNLOCK",
  HTTP2_METHOD_UPDATE: "UPDATE",
  HTTP2_METHOD_UPDATEREDIRECTREF: "UPDATEREDIRECTREF",
  HTTP2_METHOD_VERSION_CONTROL: "VERSION-CONTROL",
  HTTP_STATUS_CONTINUE: 100,
  HTTP_STATUS_SWITCHING_PROTOCOLS: 101,
  HTTP_STATUS_PROCESSING: 102,
  HTTP_STATUS_EARLY_HINTS: 103,
  HTTP_STATUS_OK: 200,
  HTTP_STATUS_CREATED: 201,
  HTTP_STATUS_ACCEPTED: 202,
  HTTP_STATUS_NON_AUTHORITATIVE_INFORMATION: 203,
  HTTP_STATUS_NO_CONTENT: 204,
  HTTP_STATUS_RESET_CONTENT: 205,
  HTTP_STATUS_PARTIAL_CONTENT: 206,
  HTTP_STATUS_MULTI_STATUS: 207,
  HTTP_STATUS_ALREADY_REPORTED: 208,
  HTTP_STATUS_IM_USED: 226,
  HTTP_STATUS_MULTIPLE_CHOICES: 300,
  HTTP_STATUS_MOVED_PERMANENTLY: 301,
  HTTP_STATUS_FOUND: 302,
  HTTP_STATUS_SEE_OTHER: 303,
  HTTP_STATUS_NOT_MODIFIED: 304,
  HTTP_STATUS_USE_PROXY: 305,
  HTTP_STATUS_TEMPORARY_REDIRECT: 307,
  HTTP_STATUS_PERMANENT_REDIRECT: 308,
  HTTP_STATUS_BAD_REQUEST: 400,
  HTTP_STATUS_UNAUTHORIZED: 401,
  HTTP_STATUS_PAYMENT_REQUIRED: 402,
  HTTP_STATUS_FORBIDDEN: 403,
  HTTP_STATUS_NOT_FOUND: 404,
  HTTP_STATUS_METHOD_NOT_ALLOWED: 405,
  HTTP_STATUS_NOT_ACCEPTABLE: 406,
  HTTP_STATUS_PROXY_AUTHENTICATION_REQUIRED: 407,
  HTTP_STATUS_REQUEST_TIMEOUT: 408,
  HTTP_STATUS_CONFLICT: 409,
  HTTP_STATUS_GONE: 410,
  HTTP_STATUS_LENGTH_REQUIRED: 411,
  HTTP_STATUS_PRECONDITION_FAILED: 412,
  HTTP_STATUS_PAYLOAD_TOO_LARGE: 413,
  HTTP_STATUS_URI_TOO_LONG: 414,
  HTTP_STATUS_UNSUPPORTED_MEDIA_TYPE: 415,
  HTTP_STATUS_RANGE_NOT_SATISFIABLE: 416,
  HTTP_STATUS_EXPECTATION_FAILED: 417,
  HTTP_STATUS_TEAPOT: 418,
  HTTP_STATUS_MISDIRECTED_REQUEST: 421,
  HTTP_STATUS_UNPROCESSABLE_ENTITY: 422,
  HTTP_STATUS_LOCKED: 423,
  HTTP_STATUS_FAILED_DEPENDENCY: 424,
  HTTP_STATUS_TOO_EARLY: 425,
  HTTP_STATUS_UPGRADE_REQUIRED: 426,
  HTTP_STATUS_PRECONDITION_REQUIRED: 428,
  HTTP_STATUS_TOO_MANY_REQUESTS: 429,
  HTTP_STATUS_REQUEST_HEADER_FIELDS_TOO_LARGE: 431,
  HTTP_STATUS_UNAVAILABLE_FOR_LEGAL_REASONS: 451,
  HTTP_STATUS_INTERNAL_SERVER_ERROR: 500,
  HTTP_STATUS_NOT_IMPLEMENTED: 501,
  HTTP_STATUS_BAD_GATEWAY: 502,
  HTTP_STATUS_SERVICE_UNAVAILABLE: 503,
  HTTP_STATUS_GATEWAY_TIMEOUT: 504,
  HTTP_STATUS_HTTP_VERSION_NOT_SUPPORTED: 505,
  HTTP_STATUS_VARIANT_ALSO_NEGOTIATES: 506,
  HTTP_STATUS_INSUFFICIENT_STORAGE: 507,
  HTTP_STATUS_LOOP_DETECTED: 508,
  HTTP_STATUS_BANDWIDTH_LIMIT_EXCEEDED: 509,
  HTTP_STATUS_NOT_EXTENDED: 510,
  HTTP_STATUS_NETWORK_AUTHENTICATION_REQUIRED: 511,
};

const kSingleValueHeaders = new Set([
  constants.HTTP2_HEADER_STATUS,
  constants.HTTP2_HEADER_METHOD,
  constants.HTTP2_HEADER_AUTHORITY,
  constants.HTTP2_HEADER_SCHEME,
  constants.HTTP2_HEADER_PATH,
  constants.HTTP2_HEADER_PROTOCOL,
  constants.HTTP2_HEADER_ACCESS_CONTROL_ALLOW_CREDENTIALS,
  constants.HTTP2_HEADER_ACCESS_CONTROL_MAX_AGE,
  constants.HTTP2_HEADER_ACCESS_CONTROL_REQUEST_METHOD,
  constants.HTTP2_HEADER_AGE,
  constants.HTTP2_HEADER_AUTHORIZATION,
  constants.HTTP2_HEADER_CONTENT_ENCODING,
  constants.HTTP2_HEADER_CONTENT_LANGUAGE,
  constants.HTTP2_HEADER_CONTENT_LENGTH,
  constants.HTTP2_HEADER_CONTENT_LOCATION,
  constants.HTTP2_HEADER_CONTENT_MD5,
  constants.HTTP2_HEADER_CONTENT_RANGE,
  constants.HTTP2_HEADER_CONTENT_TYPE,
  constants.HTTP2_HEADER_DATE,
  constants.HTTP2_HEADER_DNT,
  constants.HTTP2_HEADER_ETAG,
  constants.HTTP2_HEADER_EXPIRES,
  constants.HTTP2_HEADER_FROM,
  constants.HTTP2_HEADER_HOST,
  constants.HTTP2_HEADER_IF_MATCH,
  constants.HTTP2_HEADER_IF_MODIFIED_SINCE,
  constants.HTTP2_HEADER_IF_NONE_MATCH,
  constants.HTTP2_HEADER_IF_RANGE,
  constants.HTTP2_HEADER_IF_UNMODIFIED_SINCE,
  constants.HTTP2_HEADER_LAST_MODIFIED,
  constants.HTTP2_HEADER_LOCATION,
  constants.HTTP2_HEADER_MAX_FORWARDS,
  constants.HTTP2_HEADER_PROXY_AUTHORIZATION,
  constants.HTTP2_HEADER_RANGE,
  constants.HTTP2_HEADER_REFERER,
  constants.HTTP2_HEADER_RETRY_AFTER,
  constants.HTTP2_HEADER_TK,
  constants.HTTP2_HEADER_UPGRADE_INSECURE_REQUESTS,
  constants.HTTP2_HEADER_USER_AGENT,
  constants.HTTP2_HEADER_X_CONTENT_TYPE_OPTIONS,
]);

export function getDefaultSettings(): Record<string, unknown> {
  notImplemented("http2.getDefaultSettings");
  return {};
}

export function getPackedSettings(_settings: Record<string, unknown>): Buffer {
  notImplemented("http2.getPackedSettings");
  return {};
}

export function getUnpackedSettings(
  _buffer: Buffer | TypedArray,
): Record<string, unknown> {
  notImplemented("http2.getUnpackedSettings");
  return {};
}

export const sensitiveHeaders = Symbol("nodejs.http2.sensitiveHeaders");

export class Http2ServerRequest {
  constructor() {
  }

  get aborted(): boolean {
    notImplemented("Http2ServerRequest.aborted");
    return false;
  }

  get authority(): string {
    notImplemented("Http2ServerRequest.authority");
    return "";
  }

  get complete(): boolean {
    notImplemented("Http2ServerRequest.complete");
    return false;
  }

  get connection(): Socket /*| TlsSocket*/ {
    notImplemented("Http2ServerRequest.connection");
    return {};
  }

  destroy(_error: Error) {
    notImplemented("Http2ServerRequest.destroy");
  }

  get headers(): Record<string, unknown> {
    notImplemented("Http2ServerRequest.headers");
    return {};
  }

  get httpVersion(): string {
    notImplemented("Http2ServerRequest.httpVersion");
    return "";
  }

  get method(): string {
    notImplemented("Http2ServerRequest.method");
    return "";
  }

  get rawHeaders(): string[] {
    notImplemented("Http2ServerRequest.rawHeaders");
    return [];
  }

  get rawTrailers(): string[] {
    notImplemented("Http2ServerRequest.rawTrailers");
    return [];
  }

  get scheme(): string {
    notImplemented("Http2ServerRequest.scheme");
    return "";
  }

  setTimeout(msecs: number, callback?: () => unknown) {
    this.stream.setTimeout(callback, msecs);
  }

  get socket(): Socket /*| TlsSocket*/ {
    notImplemented("Http2ServerRequest.socket");
    return {};
  }

  get stream(): Http2Stream {
    notImplemented("Http2ServerRequest.stream");
    return new Http2Stream();
  }

  get trailers(): Record<string, unknown> {
    notImplemented("Http2ServerRequest.trailers");
    return {};
  }

  get url(): string {
    notImplemented("Http2ServerRequest.url");
    return "";
  }
}

export class Http2ServerResponse {
  constructor() {
  }

  addTrailers(_headers: Record<string, unknown>) {
    notImplemented("Http2ServerResponse.addTrailers");
  }

  get connection(): Socket /*| TlsSocket*/ {
    notImplemented("Http2ServerResponse.connection");
    return {};
  }

  createPushResponse(
    _headers: Record<string, unknown>,
    _callback: () => unknown,
  ) {
    notImplemented("Http2ServerResponse.createPushResponse");
  }

  end(
    _data: string | Buffer | Uint8Array,
    _encoding: string,
    _callback: () => unknown,
  ) {
    notImplemented("Http2ServerResponse.end");
  }

  get finished(): boolean {
    notImplemented("Http2ServerResponse.finished");
    return false;
  }

  getHeader(_name: string): string {
    notImplemented("Http2ServerResponse.getHeader");
    return "";
  }

  getHeaderNames(): string[] {
    notImplemented("Http2ServerResponse.getHeaderNames");
    return [];
  }

  getHeaders(): Record<string, unknown> {
    notImplemented("Http2ServerResponse.getHeaders");
    return {};
  }

  hasHeader(_name: string) {
    notImplemented("Http2ServerResponse.hasHeader");
  }

  get headersSent(): boolean {
    notImplemented("Http2ServerResponse.headersSent");
    return false;
  }

  removeHeader(_name: string) {
    notImplemented("Http2ServerResponse.removeHeader");
  }

  get req(): Http2ServerRequest {
    notImplemented("Http2ServerResponse.req");
    return new Http2ServerRequest();
  }

  get sendDate(): boolean {
    notImplemented("Http2ServerResponse.sendDate");
    return false;
  }

  setHeader(_name: string, _value: string | string[]) {
    notImplemented("Http2ServerResponse.setHeader");
  }

  setTimeout(msecs: number, callback?: () => unknown) {
    this.stream.setTimeout(msecs, callback);
  }

  get socket(): Socket /*| TlsSocket*/ {
    notImplemented("Http2ServerResponse.socket");
    return {};
  }

  get statusCode(): number {
    notImplemented("Http2ServerResponse.statusCode");
    return 0;
  }

  get statusMessage(): string {
    notImplemented("Http2ServerResponse.statusMessage");
    return "";
  }

  get stream(): Http2Stream {
    notImplemented("Http2ServerResponse.stream");
    return new Http2Stream();
  }

  get writableEnded(): boolean {
    notImplemented("Http2ServerResponse.writableEnded");
    return false;
  }

  write(
    _chunk: string | Buffer | Uint8Array,
    _encoding: string,
    _callback: () => unknown,
  ) {
    notImplemented("Http2ServerResponse.write");
    return this.write;
  }

  writeContinue() {
    notImplemented("Http2ServerResponse.writeContinue");
  }

  writeEarlyHints(_hints: Record<string, unknown>) {
    notImplemented("Http2ServerResponse.writeEarlyHints");
  }

  writeHead(
    _statusCode: number,
    _statusMessage: string,
    _headers: Record<string, unknown>,
  ) {
    notImplemented("Http2ServerResponse.writeHead");
  }
}

export default {
  createServer,
  createSecureServer,
  connect,
  constants,
  getDefaultSettings,
  getPackedSettings,
  getUnpackedSettings,
  sensitiveHeaders,
  Http2ServerRequest,
  Http2ServerResponse,
};
